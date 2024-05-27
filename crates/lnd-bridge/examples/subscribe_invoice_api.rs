use anyhow::Result;
use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,lnd_bridge=trace")
        .init();

    let macaroon = "[enter macaroon here]".to_string();
    let lnd_bridge = lnd_bridge::LndBridge::new("localhost:18080".to_string(), macaroon, false);

    let r_hash = "UPJS32pkCZlzhMAYsEYnPkMq0AD8Vnnd6BnHcGQnvBw=".to_string();

    let mut stream = lnd_bridge.subscribe_to_invoice(r_hash);

    while let Some(result) = stream.try_next().await? {
        tracing::info!("{result:?}");
    }

    Ok(())
}
