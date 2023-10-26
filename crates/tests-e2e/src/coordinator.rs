use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Txid;
use coordinator::admin::Balance;
use coordinator::routes::InvoiceParams;
use coordinator_commons::CollaborativeRevert;
use ln_dlc_node::lightning_invoice;
use ln_dlc_node::node::NodeInfo;
use reqwest::Client;
use rust_decimal_macros::dec;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

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
    pub state: SubChannelState,
}
#[derive(Deserialize, Debug)]
pub struct Channel {
    pub channel_id: String,
    pub counterparty: String,
    #[serde(deserialize_with = "null_to_default")]
    pub original_funding_txo: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub enum SubChannelState {
    Signed,
    Closing,
    OnChainClosed,
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

    pub async fn pay_invoice(&self, invoice: &str) -> Result<()> {
        self.post(&format!("/api/admin/send_payment/{invoice}"))
            .await?;
        Ok(())
    }

    pub async fn create_invoice(&self, amount: Option<u64>) -> Result<lightning_invoice::Invoice> {
        let invoice_params = InvoiceParams {
            amount,
            description: Some("Fee for tests".to_string()),
            expiry: None,
        };

        let encoded_params = serde_urlencoded::to_string(&invoice_params)?;

        let invoice = self
            .get(&format!("/api/invoice?{encoded_params}"))
            .await?
            .text()
            .await?
            .parse()?;

        Ok(invoice)
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

    pub async fn force_close_channel(&self, channel_id: &str) -> Result<reqwest::Response> {
        self.delete(format!("/api/admin/channels/{channel_id}?force=true").as_str())
            .await
    }

    pub async fn collaborative_revert_channel(
        &self,
        channel_id: &str,
        txid: Txid,
        vout: u32,
    ) -> Result<reqwest::Response> {
        self.post_with_body(
            "/api/admin/channels/revert",
            &CollaborativeRevert {
                channel_id: channel_id.to_string(),
                price: dec!(30_000.0),
                fee_rate_sats_vb: 4,
                txid,
                vout,
            },
        )
        .await
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

    async fn post_with_body<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        self.client
            .post(format!("{0}{path}", self.host))
            .json(body)
            .send()
            .await
            .context("Could not send POST request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }

    async fn delete(&self, path: &str) -> Result<reqwest::Response> {
        self.client
            .delete(format!("{0}{path}", self.host))
            .send()
            .await
            .context("Could not send DELETE request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }
}

fn null_to_default<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    let key = Option::<T>::deserialize(de)?;
    Ok(key.unwrap_or_default())
}
