use anyhow::Result;
use futures::TryStreamExt;
use orderbook_commons::Signature;
use secp256k1::SecretKey;
use secp256k1::SECP256K1;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,orderbook_client=trace")
        .init();
    let secret_key = SecretKey::from_slice(&b"bring sally up, bring sally down"[..]).unwrap();

    let url = "ws://localhost:8000/api/orderbook/websocket".to_string();

    let authenticate = move |msg| {
        let signature = secret_key.sign_ecdsa(msg);
        Signature {
            pubkey: secret_key.public_key(SECP256K1),
            signature,
        }
    };
    let mut stream = orderbook_client::subscribe_with_authentication(url, &authenticate);

    while let Some(result) = stream.try_next().await? {
        tracing::info!("Received: {result}");
    }

    Ok(())
}
