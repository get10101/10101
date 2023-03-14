use anyhow::Context;
pub use anyhow::Error;
use anyhow::Result;
use async_stream::stream;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use orderbook_commons::create_sign_message;
use orderbook_commons::Signature;
use secp256k1::Message;
use serde::Serialize;
use serde_json::to_string;
use tokio_tungstenite::tungstenite;

/// Connects to the 10101 orderbook websocket API
///
/// If the connection needs authentication please use `subscribe_with_authentication` instead.
pub fn subscribe(url: String) -> impl Stream<Item = Result<String, Error>> + Unpin {
    subscribe_impl(None, url)
}

/// Connects to the orderbook websocket API with authentication
///
/// It subscribes and yields all messages.
pub fn subscribe_with_authentication(
    url: String,
    authenticate: impl Fn(Message) -> Signature,
) -> impl Stream<Item = Result<String, Error>> + Unpin {
    let signature = authenticate(create_sign_message());
    subscribe_impl(Some(signature), url)
}

/// Connects to the orderbook websocket API yields all messages.
fn subscribe_impl(
    signature: Option<Signature>,
    url: String,
) -> impl Stream<Item = Result<String, Error>> + Unpin {
    let stream = stream! {
        tracing::debug!("Connecting to orderbook API");

        let (mut connection, _) = tokio_tungstenite::connect_async(url.clone())
            .await.context("Could not connect to websocket")?;

        tracing::info!("Connected to orderbook realtime API");


        if let Some(signature) = signature {
            let _ = connection
                .send(tungstenite::Message::try_from(Command::from(signature))?)
                .await;
        }


        loop {
            tokio::select! {
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

#[derive(Debug, Serialize)]
pub enum Command {
    Authenticate(Signature),
}

impl TryFrom<Command> for tungstenite::Message {
    type Error = Error;

    fn try_from(command: Command) -> Result<Self, Self::Error> {
        let msg = to_string(&command)?;
        Ok(tungstenite::Message::Text(msg))
    }
}

impl From<Signature> for Command {
    fn from(sig: Signature) -> Self {
        Command::Authenticate(sig)
    }
}

#[cfg(test)]
mod test {
    use crate::create_sign_message;
    use secp256k1::SecretKey;
    use secp256k1::SECP256K1;
    use std::str::FromStr;

    #[test]
    fn test_signature_get() {
        let secret_key = test_secret_key();

        let message = create_sign_message();
        let signature = secret_key.sign_ecdsa(message);

        let should_signature = secp256k1::ecdsa::Signature::from_str(
            "304402202f2545f818a5dac9311157d75065156b141e5a6437e817d1d75f9fab084e46940220757bb6f0916f83b2be28877a0d6b05c45463794e3c8c99f799b774443575910d",
        )
        .unwrap();
        assert_eq!(signature, should_signature);
    }

    fn test_secret_key() -> SecretKey {
        SecretKey::from_slice(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 27, 29, 30, 31,
        ])
        .unwrap()
    }

    #[test]
    fn test_verify_signature() {
        let secret_key = test_secret_key();

        let message = create_sign_message();
        let signature = secret_key.sign_ecdsa(message);

        let pubkey = secret_key.public_key(SECP256K1);

        let msg = create_sign_message();

        signature.verify(&msg, &pubkey).unwrap();
    }
}
