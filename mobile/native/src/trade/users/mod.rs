use crate::commons::reqwest_client;
use crate::config;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use commons::RegisterParams;
use commons::UpdateUsernameParams;
use commons::User;

/// Enroll the user in the beta program
pub async fn register_beta(
    contact: String,
    version: String,
    referral_code: Option<String>,
) -> Result<()> {
    let name = crate::names::get_new_name();
    let register = RegisterParams {
        pubkey: ln_dlc::get_node_pubkey(),
        contact: Some(contact),
        nickname: Some(name),
        version: Some(version.clone()),
        referral_code,
    };

    tracing::debug!(
        pubkey = register.pubkey.to_string(),
        contact = register.contact,
        referral_code = register.referral_code,
        version,
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
    tracing::info!("Received user details {user:?}");

    Ok(user)
}

/// Update a user's name on the coordinator
pub async fn update_username(name: String) -> Result<()> {
    let update_nickname = UpdateUsernameParams {
        pubkey: ln_dlc::get_node_pubkey(),
        nickname: Some(name),
    };

    tracing::debug!(
        pubkey = update_nickname.pubkey.to_string(),
        nickname = update_nickname.nickname,
        "Updating user nickname"
    );

    let client = reqwest_client();
    let response = client
        .put(format!(
            "http://{}/api/users/nickname",
            config::get_http_endpoint()
        ))
        .json(&update_nickname)
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
    tracing::info!("Updated user nickname successfully");
    Ok(())
}
