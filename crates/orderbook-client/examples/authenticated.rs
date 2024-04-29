use anyhow::Result;
use futures::never::Never;
use futures::TryStreamExt;
use secp256k1::SecretKey;
use secp256k1::SECP256K1;
use std::time::Duration;
use xxi_node::commons::Signature;

#[tokio::main]
async fn main() -> Result<Never> {
    tracing_subscriber::fmt()
        .with_env_filter("info,orderbook_client=trace")
        .init();
    let secret_key =
        SecretKey::from_slice(&b"bring sally up, bring sally down"[..]).expect("valid secret key");

    let url = "ws://localhost:8000/api/orderbook/websocket".to_string();

    let authenticate = move |msg| {
        let signature = secret_key.sign_ecdsa(msg);
        Signature {
            pubkey: secret_key.public_key(SECP256K1),
            signature,
        }
    };

    loop {
        let (_, mut stream) =
            orderbook_client::subscribe_with_authentication(url.clone(), &authenticate, None, None)
                .await?;

        loop {
            match stream.try_next().await {
                Ok(Some(event)) => tracing::info!(%event, "Event received"),
                Ok(None) => {
                    tracing::error!("Stream ended");
                    break;
                }
                Err(error) => {
                    tracing::error!(%error, "Stream ended");
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
