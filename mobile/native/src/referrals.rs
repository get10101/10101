use crate::commons::reqwest_client;
use crate::config;
use anyhow::Result;
use commons::ReferralStatus;
use reqwest::Url;

pub(crate) async fn get_referral_status() -> Result<ReferralStatus> {
    let node = crate::state::get_node();
    let client = reqwest_client();
    let url = format!("http://{}", config::get_http_endpoint());
    let url = Url::parse(&url).expect("correct URL");
    let url = url.join(format!("/api/users/{}/referrals", node.inner.info.pubkey).as_str())?;
    let response = client.get(url).send().await?;
    let referral_status = response.json::<ReferralStatus>().await?;
    tracing::debug!(referral_status = ?referral_status, "Fetched referral status");

    Ok(referral_status)
}
