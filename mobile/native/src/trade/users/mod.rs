use crate::commons::reqwest_client;
use crate::config;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use commons::RegisterParams;
use commons::User;

/// Enroll the user in the beta program
pub async fn register_beta(contact: String) -> Result<()> {
    let register = RegisterParams {
        pubkey: ln_dlc::get_node_pubkey(),
        contact: Some(contact),
    };

    tracing::debug!(
        pubkey = register.pubkey.to_string(),
        contact = register.contact,
        "Registering user"
    );

    let client = reqwest_client();
    let response = client
        .post(format!("http://{}/api/users", config::get_http_endpoint()))
        .json(&register)
        .send()
        .await
        .context("Failed to register beta program with coordinator")?;

    let status_code = response.status();
    if !status_code.is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };
        return Err(anyhow!(
            "Could not register with coordinator: HTTP${status_code}: {response_text}"
        ));
    }
    tracing::info!("Registered into beta program successfully");
    Ok(())
}

/// Retrieve latest user details
pub async fn get_user_details() -> Result<User> {
    let key = ln_dlc::get_node_pubkey();

    let client = reqwest_client();
    let response = client
        .get(format!(
            "http://{}/api/users/{}",
            config::get_http_endpoint(),
            key
        ))
        .send()
        .await
        .context("Failed to retrieve user details")?;

    let user = response.json::<User>().await?;

    Ok(user)
}
