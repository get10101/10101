use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,lnd_bridge=trace")
        .init();

    let macaroon = "[enter macroon here]".to_string();
    let lnd_bridge = lnd_bridge::LndBridge::new("localhost:18080".to_string(), macaroon, false);

    let payment_hash = "".to_string();
    lnd_bridge.cancel_invoice(payment_hash).await?;

    Ok(())
}
