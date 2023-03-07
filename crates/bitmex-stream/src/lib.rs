use crate::tungstenite::http::Method;
use anyhow::Context;
use async_stream::stream;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use serde::ser::SerializeTuple;
use serde::Serialize;
use serde_json::to_string;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio_tungstenite::tungstenite;
use tracing::Instrument;
use url::Url;

pub use anyhow::Error;

/// Connects to the BitMex websocket API
///
/// It subscribes to the specified topics (comma-separated) and yields all messages.
/// If the topics need authentication please use `subscribe_with_credentials` instead.
pub fn subscribe<const N: usize>(
    topics: [String; N],
    network: Network,
) -> impl Stream<Item = Result<String, Error>> + Unpin {
    subscribe_impl(topics, network, None)
}

/// Connects to the BitMex websocket API with authentication
///
/// It subscribes to the specified topics (comma-separated) and yields all messages.
/// If invalid credentials have been provided but a topic was provided which needs authentication
/// the stream will be closed.
pub fn subscribe_with_credentials<const N: usize>(
    topics: [String; N],
    network: Network,
    credentials: Credentials,
) -> impl Stream<Item = Result<String, Error>> + Unpin {
    subscribe_impl(topics, network, Some(credentials))
}

/// Connects to the BitMex websocket API, subscribes to the specified topics (comma-separated) and
/// yields all messages.
///
/// To keep the connection alive, a websocket `Ping` is sent every 5 seconds in case no other
/// message was received in-between. This is according to BitMex's API documentation: https://www.bitmex.com/app/wsAPI#Heartbeats
fn subscribe_impl<const N: usize>(
    topics: [String; N],
    network: Network,
    credentials: Option<Credentials>,
) -> impl Stream<Item = Result<String, Error>> + Unpin {
    let url = network.to_url();
    let url = format!("wss://{url}/realtime");

    let stream = stream! {
        tracing::debug!("Connecting to BitMex realtime API");

        let (mut connection, _) = tokio_tungstenite::connect_async(url.clone())
            .await.context("Could not connect to websocket")?;

        tracing::info!("Connected to BitMex realtime API");

        if let Some(credentials) = credentials {
            let start = SystemTime::now();
            let expires = start
                .duration_since(UNIX_EPOCH)?
                .add(Duration::from_secs(5))
                .as_secs();
            let signature = credentials.sign(Method::GET, expires, &Url::parse(url.as_str())?, "");
            let _ = connection
                .send(tungstenite::Message::try_from(Command::from(signature))?)
                .await;

        }
        let _ = connection
                .send(tungstenite::Message::try_from(Command::Subscribe(
            topics.to_vec(),
        ))?)
        .await;


        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    let span = tracing::trace_span!("Ping BitMex");
                    span.in_scope(|| tracing::trace!("No message from BitMex in the last 5 seconds, pinging"));
                    let _ = connection
                        .send(tungstenite::Message::Ping([0u8; 32].to_vec()))
                        .instrument(span)
                        .await;
                },
                msg = connection.next() => {
                    let msg = match msg {
                        Some(Ok(msg)) => {
                            msg
                        },
                        None => {
                            return;
                        }
                        Some(Err(e)) => {
                            yield Err(anyhow::anyhow!(e));
                            return;
                        }
                    };

                    match msg {
                        tungstenite::Message::Pong(_) => {
                            tracing::trace!("Received pong");
                            continue;
                        }
                        tungstenite::Message::Text(text) => {
                            yield Ok(text);
                        }
                        other => {
                            tracing::trace!("Unsupported message: {:?}", other);
                            continue;
                        }
                    }
                }
            }
        }
    };

    stream.boxed()
}

#[derive(Debug, Copy, Clone)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    pub fn to_url(&self) -> String {
        match self {
            Network::Mainnet => "www.bitmex.com".to_string(),
            Network::Testnet => "testnet.bitmex.com".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", content = "args")]
#[serde(rename_all = "camelCase")]
pub enum Command {
    Subscribe(Vec<String>),
    #[serde(rename = "authKeyExpires")]
    Authenticate(Signature),
}

impl TryFrom<Command> for tungstenite::Message {
    type Error = Error;

    fn try_from(command: Command) -> Result<Self, Self::Error> {
        let msg = to_string(&command)?;
        Ok(tungstenite::Message::Text(msg))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Credentials {
    pub api_key: String,
    pub secret: String,
}

#[derive(Debug)]
pub struct Signature {
    api_key: String,
    signature: String,
    expires: u64,
}

impl Credentials {
    pub fn new(api_key: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            secret: secret.into(),
        }
    }

    fn sign(&self, method: Method, expires: u64, url: &Url, body: &str) -> Signature {
        let signed_key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, self.secret.as_bytes());
        let sign_message = match url.query() {
            Some(query) => format!(
                "{}{}?{}{}{}",
                method.as_str(),
                url.path(),
                query,
                expires,
                body
            ),
            None => format!("{}{}{}{}", method.as_str(), url.path(), expires, body),
        };

        let signature = hex::encode(ring::hmac::sign(&signed_key, sign_message.as_bytes()));
        Signature {
            api_key: self.api_key.clone(),
            signature,
            expires,
        }
    }
}

impl From<Signature> for Command {
    fn from(sig: Signature) -> Self {
        Command::Authenticate(sig)
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(&self.api_key)?;
        tup.serialize_element(&self.expires)?;
        tup.serialize_element(&self.signature)?;
        tup.end()
    }
}

#[cfg(test)]
mod test {
    use super::Credentials;
    use crate::tungstenite::http::Method;
    use crate::Signature;
    use anyhow::Result;
    use serde_json::to_string;
    use url::Url;

    #[test]
    fn test_signature_get() -> Result<()> {
        let tr = Credentials::new(
            "LAqUlngMIQkIUjXMUreyu3qn",
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
        );
        let Signature { signature, .. } = tr.sign(
            Method::GET,
            1518064236,
            &Url::parse("http://a.com/api/v1/instrument")?,
            "",
        );
        assert_eq!(
            signature,
            "c7682d435d0cfe87c16098df34ef2eb5a549d4c5a3c2b1f0f77b8af73423bf00"
        );
        Ok(())
    }

    #[test]
    fn test_signature_get_param() -> Result<()> {
        let tr = Credentials::new(
            "LAqUlngMIQkIUjXMUreyu3qn",
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
        );
        let Signature { signature, .. } = tr.sign(
            Method::GET,
            1518064237,
            &Url::parse_with_params(
                "http://a.com/api/v1/instrument",
                &[("filter", r#"{"symbol": "XBTM15"}"#)],
            )?,
            "",
        );
        assert_eq!(
            signature,
            "e2f422547eecb5b3cb29ade2127e21b858b235b386bfa45e1c1756eb3383919f"
        );
        Ok(())
    }

    #[test]
    fn test_signature_post() -> Result<()> {
        let credentials = Credentials::new(
            "LAqUlngMIQkIUjXMUreyu3qn",
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
        );
        let Signature {  signature, .. } = credentials.sign(
            Method::POST,
            1518064238,
            &Url::parse("http://a.com/api/v1/order")?,
            r#"{"symbol":"XBTM15","price":219.0,"clOrdID":"mm_bitmex_1a/oemUeQ4CAJZgP3fjHsA","orderQty":98}"#,
        );
        assert_eq!(
            signature,
            "1749cd2ccae4aa49048ae09f0b95110cee706e0944e6a14ad0b3a8cb45bd336b"
        );
        Ok(())
    }

    #[test]
    fn test_serialize_signature() {
        let sig = Signature {
            api_key: "api_key123".to_string(),
            signature: "signature0x42".to_string(),
            expires: 42,
        };
        let serialized = to_string(&sig).unwrap();
        assert_eq!(serialized, r#"["api_key123",42,"signature0x42"]"#);
    }
}
