use anyhow::Result;
use futures::TryStreamExt;
use orderbook_client::Credentials;
use secp256k1::SecretKey;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,orderbook_client=trace")
        .init();
    let secret_key = SecretKey::from_slice(&b"bring sally up, bring sally down"[..]).unwrap();

    let url = "ws://localhost:8000/api/orderbook/websocket".to_string();

    let mut stream =
        orderbook_client::subscribe_with_authentication(url, Credentials { secret_key });

    while let Some(result) = stream.try_next().await? {
        tracing::info!("Received: {result}");
    }

    Ok(())
}
