use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use reqwest::Client;
use serde::Deserialize;

/// A wrapper over the coordinator HTTP API.
///
/// It does not aim to be complete, functionality will be added as needed.
pub struct Coordinator {
    client: Client,
    host: String,
}

impl Coordinator {
    pub fn new(client: Client, host: &str) -> Self {
        Self {
            client,
            host: host.to_string(),
        }
    }

    pub fn new_local(client: Client) -> Self {
        Self::new(client, "http://localhost:8000")
    }

    /// Check whether the coordinator is running.
    pub async fn is_running(&self) -> bool {
        self.get("/health").await.is_ok()
    }

    pub async fn sync_wallet(&self) -> Result<()> {
        self.post("/api/admin/sync").await?;
        Ok(())
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        Ok(self.get("/api/newaddress").await?.text().await?.parse()?)
    }

    pub async fn get_dlc_channels(&self) -> Result<Vec<DlcChannelDetails>> {
        Ok(self.get("/api/admin/dlc_channels").await?.json().await?)
    }

    pub async fn rollover(&self, dlc_channel_id: &str) -> Result<reqwest::Response> {
        self.post(format!("/api/rollover/{dlc_channel_id}").as_str())
            .await
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.client
            .get(format!("{0}{path}", self.host))
            .send()
            .await
            .context("Could not send GET request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }

    async fn post(&self, path: &str) -> Result<reqwest::Response> {
        self.client
            .post(format!("{0}{path}", self.host))
            .send()
            .await
            .context("Could not send POST request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }
}

#[derive(Deserialize, Debug)]
pub struct DlcChannels {
    #[serde(flatten)]
    pub channels: Vec<DlcChannelDetails>,
}

#[derive(Deserialize, Debug)]
pub struct DlcChannelDetails {
    pub dlc_channel_id: Option<String>,
    pub counter_party: String,
    pub channel_state: ChannelState,
    pub signed_channel_state: Option<SignedChannelState>,
    pub update_idx: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub enum ChannelState {
    Offered,
    Accepted,
    Signed,
    Closing,
    Closed,
    CounterClosed,
    ClosedPunished,
    CollaborativelyClosed,
    FailedAccept,
    FailedSign,
}

#[derive(Deserialize, Debug)]
pub enum SignedChannelState {
    Established,
    SettledOffered,
    SettledReceived,
    SettledAccepted,
    SettledConfirmed,
    Settled,
    RenewOffered,
    RenewAccepted,
    RenewConfirmed,
    RenewFinalized,
    Closing,
    CollaborativeCloseOffered,
}
