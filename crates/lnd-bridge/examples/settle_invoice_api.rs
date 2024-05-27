use anyhow::Result;
use base64::engine::general_purpose;
use base64::Engine;
use sha2::Digest;
use sha2::Sha256;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,lnd_bridge=trace")
        .init();

    let macaroon = "[enter macaroon here]".to_string();
    let lnd_bridge = lnd_bridge::LndBridge::new("localhost:18080".to_string(), macaroon, false);

    let pre_image = "5PfDXnydoLscQ2qk-0WR94TY9zWAXMcN8A2-0NW2RJw=".to_string();

    let mut hasher = Sha256::new();
    hasher.update(pre_image.clone());

    let hash = hasher.finalize();
    let hash = general_purpose::STANDARD.encode(hash);

    tracing::info!("r_hash: {hash}");

    lnd_bridge.settle_invoice(pre_image).await?;

    Ok(())
}
