use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct NodeInfo {
    pub pubkey: PublicKey,
    pub address: SocketAddr,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InvoiceParams {
    pub amount: Option<u64>,
    pub description: Option<String>,
    pub expiry: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Balance {
    pub offchain: u64,
    pub onchain: u64,
}

/// A wrapper over the coordinator HTTP API.
///
/// It does not aim to be complete, functionality will be added as needed.
pub struct Coordinator {
    client: Client,
    host: String,
}

#[derive(Deserialize)]
pub struct DlcChannels {
    #[serde(flatten)]
    pub channel_details: Vec<DlcChannel>,
}

#[derive(Deserialize, Debug)]
pub struct DlcChannel {
    pub channel_id: String,
    pub dlc_channel_id: Option<String>,
    pub counter_party: String,
    pub subchannel_state: SubChannelState,
}
#[derive(Deserialize, Debug)]
pub struct Channel {
    pub channel_id: String,
    pub counterparty: String,
    pub funding_txo: Option<String>,
    pub original_funding_txo: Option<String>,
    pub outbound_capacity_msat: u64,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub enum SubChannelState {
    Signed,
    Closing,
    OnChainClosed,
    CounterOnChainClosed,
    CloseConfirmed,
    // We don't care about other states for now
    #[serde(other)]
    Other,
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

    pub async fn is_node_connected(&self, node_id: &str) -> Result<bool> {
        let result = self
            .get(&format!("/api/admin/is_connected/{node_id}"))
            .await?
            .status()
            .is_success();
        Ok(result)
    }

    pub async fn sync_wallet(&self) -> Result<()> {
        self.post("/api/admin/sync").await?;
        Ok(())
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        Ok(self.get("/api/newaddress").await?.text().await?.parse()?)
    }

    pub async fn get_balance(&self) -> Result<Balance> {
        Ok(self.get("/api/admin/balance").await?.json().await?)
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo> {
        self.get("/api/node")
            .await?
            .json()
            .await
            .context("could not parse json")
    }

    pub async fn broadcast_node_announcement(&self) -> Result<reqwest::Response> {
        let status = self
            .post("/api/admin/broadcast_announcement")
            .await?
            .error_for_status()?;
        Ok(status)
    }

    pub async fn get_dlc_channels(&self) -> Result<Vec<DlcChannel>> {
        Ok(self.get("/api/admin/dlc_channels").await?.json().await?)
    }

    pub async fn get_channels(&self) -> Result<Vec<Channel>> {
        Ok(self.get("/api/admin/channels").await?.json().await?)
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
