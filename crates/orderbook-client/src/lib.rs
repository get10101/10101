use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use async_stream::stream;
use futures::stream::SplitSink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use secp256k1::Message;
use tokio_tungstenite_wasm as tungstenite;
use tokio_tungstenite_wasm::WebSocketStream;
use xxi_node::commons::create_sign_message;
use xxi_node::commons::OrderbookRequest;
use xxi_node::commons::Signature;
use xxi_node::commons::AUTH_SIGN_MESSAGE;

/// Connects to the 10101 orderbook WebSocket API.
///
/// If the connection needs authentication please use `subscribe_with_authentication` instead.
pub async fn subscribe(
    url: String,
) -> Result<(
    SplitSink<WebSocketStream, tungstenite::Message>,
    impl Stream<Item = Result<String, anyhow::Error>> + Unpin,
)> {
    subscribe_impl(None, url, None, None).await
}

/// Connects to the orderbook WebSocket API with authentication.
///
/// It subscribes and yields all messages.
pub async fn subscribe_with_authentication(
    url: String,
    authenticate: impl Fn(Message) -> Signature,
    fcm_token: Option<String>,
    version: Option<String>,
) -> Result<(
    SplitSink<WebSocketStream, tungstenite::Message>,
    impl Stream<Item = Result<String, anyhow::Error>> + Unpin,
)> {
    let signature = create_auth_message_signature(authenticate);
    subscribe_impl(Some(signature), url, fcm_token, version).await
}

pub fn create_auth_message_signature(authenticate: impl Fn(Message) -> Signature) -> Signature {
    authenticate(create_sign_message(AUTH_SIGN_MESSAGE.to_vec()))
}

/// Connects to the orderbook WebSocket API and yields all messages.
async fn subscribe_impl(
    signature: Option<Signature>,
    url: String,
    fcm_token: Option<String>,
    version: Option<String>,
) -> Result<(
    SplitSink<WebSocketStream, tungstenite::Message>,
    impl Stream<Item = Result<String>> + Unpin,
)> {
    tracing::debug!("Connecting to orderbook API");

    let mut connection = tokio_tungstenite_wasm::connect(url.clone())
        .await
        .context("Could not connect to websocket")?;

    tracing::info!("Connected to orderbook realtime API");

    if let Some(signature) = signature {
        let _ = connection
            .send(tungstenite::Message::try_from(
                OrderbookRequest::Authenticate {
                    fcm_token,
                    version,
                    signature,
                },
            )?)
            .await;
    }

    let (sink, mut stream) = connection.split();

    let stream = stream! {
        loop {
            tokio::select! {
                msg = stream.next() => {
                    let msg = match msg {
                        Some(Ok(msg)) => {
                            msg
                        },
                        None => {
                            return;
                        }
                        Some(Err(e)) => {
                            yield Err(anyhow!(e));
                            return;
                        }
                    };

                    match msg {
                        tungstenite::Message::Text(s) if s.eq_ignore_ascii_case("pong") => {
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

    Ok((sink, stream.boxed()))
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

        let message = create_sign_message(b"Hello it's me Mario".to_vec());
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

        let message = create_sign_message(b"Hello it's me Mario".to_vec());
        let signature = secret_key.sign_ecdsa(message);

        let pubkey = secret_key.public_key(SECP256K1);

        let msg = create_sign_message(b"Hello it's me Mario".to_vec());

        signature.verify(&msg, &pubkey).unwrap();
    }
}
