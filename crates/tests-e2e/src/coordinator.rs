use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;

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

    pub async fn sync_node(&self) -> Result<()> {
        self.post::<()>("/api/admin/sync", None).await?;
        Ok(())
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        Ok(self.get("/api/newaddress").await?.text().await?.parse()?)
    }

    pub async fn get_dlc_channels(&self) -> Result<Vec<DlcChannelDetails>> {
        Ok(self.get("/api/admin/dlc_channels").await?.json().await?)
    }

    pub async fn rollover(&self, dlc_channel_id: &str) -> Result<reqwest::Response> {
        self.post::<()>(format!("/api/rollover/{dlc_channel_id}").as_str(), None)
            .await
    }

    pub async fn collaborative_revert(
        &self,
        request: CollaborativeRevertCoordinatorRequest,
    ) -> Result<()> {
        self.post("/api/admin/channels/revert", Some(request))
            .await?;

        Ok(())
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

    async fn post<T: Serialize>(&self, path: &str, body: Option<T>) -> Result<reqwest::Response> {
        let request = self.client.post(format!("{0}{path}", self.host));

        let request = match body {
            Some(ref body) => {
                let body = serde_json::to_string(body)?;
                request
                    .header("Content-Type", "application/json")
                    .body(body)
            }
            None => request,
        };

        request
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
    Cancelled,
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

#[derive(Serialize)]
pub struct CollaborativeRevertCoordinatorRequest {
    pub channel_id: String,
    pub fee_rate_sats_vb: u64,
    pub counter_payout: u64,
    pub price: Decimal,
}
