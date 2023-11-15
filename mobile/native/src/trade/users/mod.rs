use crate::commons::reqwest_client;
use crate::config;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use coordinator_commons::RegisterParams;

/// Enroll the user in the beta program
pub async fn register_beta(email: String) -> Result<()> {
    let register = RegisterParams {
        pubkey: ln_dlc::get_node_pubkey(),
        email: Some(email),
        nostr: None,
    };

    let client = reqwest_client();
    let response = client
        .post(format!(
            "http://{}/api/register",
            config::get_http_endpoint()
        ))
        .json(&register)
        .send()
        .await
        .context("Failed to register beta program with coordinator")?;

    if !response.status().is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };
        return Err(anyhow!(
            "Could not register email with coordinator: {response_text}"
        ));
    }
    tracing::info!("Registered into beta program successfully");
    Ok(())
}
