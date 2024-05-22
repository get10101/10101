use anyhow::Result;
use base64::engine::general_purpose;
use base64::Engine;
use lnd_bridge::InvoiceParams;
use rand::Rng;
use sha2::Digest;
use sha2::Sha256;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,lnd_bridge=trace")
        .init();

    let macaroon = "[enter macroon here]".to_string();
    let lnd_bridge = lnd_bridge::LndBridge::new("localhost:18080".to_string(), macaroon, false);

    let mut rng = rand::thread_rng();
    let pre_image: [u8; 32] = rng.gen();

    tracing::info!("{pre_image:?}");

    let mut hasher = Sha256::new();
    hasher.update(pre_image);

    let r_hash = hasher.finalize();
    let r_hash = general_purpose::STANDARD.encode(r_hash);

    let pre_image = general_purpose::URL_SAFE.encode(pre_image);

    tracing::info!("pre_image: {pre_image}");
    tracing::info!("r_hash: {r_hash}");

    let params = InvoiceParams {
        value: 10101,
        memo: "Fund your 10101 position".to_string(),
        expiry: 5 * 60, // 5 minutes
        hash: r_hash,
    };

    let response = lnd_bridge.create_invoice(params).await?;

    tracing::info!("Response: {response:?}");

    Ok(())
}
