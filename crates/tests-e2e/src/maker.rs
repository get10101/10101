use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use coordinator::routes::InvoiceParams;
use ln_dlc_node::lightning_invoice::Bolt11Invoice;
use ln_dlc_node::node::NodeInfo;
use maker::routes::Balance;
use maker::routes::ChannelParams;
use maker::routes::TargetInfo;
use reqwest::Client;
use serde::Serialize;

/// A wrapper over the maker HTTP API.
///
/// It does not aim to be complete, functionality will be added as needed.
pub struct Maker {
    client: Client,
    host: String,
}

impl Maker {
    pub fn new(client: Client, host: &str) -> Self {
        Self {
            client,
            host: host.to_string(),
        }
    }

    pub fn new_local(client: Client) -> Self {
        Self::new(client, "http://localhost:18000")
    }

    /// Check whether maker is running and that it is connected to all services
    /// it depends on
    pub async fn is_running(&self) -> bool {
        self.get("/health")
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    pub async fn sync_on_chain(&self) -> Result<()> {
        let no_json: Option<()> = None;
        self.post("/api/sync-on-chain", no_json).await?;
        Ok(())
    }

    pub async fn pay_invoice(&self, invoice: Bolt11Invoice) -> Result<()> {
        let no_json: Option<()> = None;
        self.post(&format!("/api/pay-invoice/{invoice}"), no_json)
            .await?;
        Ok(())
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        Ok(self.get("/api/newaddress").await?.text().await?.parse()?)
    }

    pub async fn get_balance(&self) -> Result<Balance> {
        Ok(self.get("/api/balance").await?.json().await?)
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo> {
        self.get("/api/node")
            .await?
            .json()
            .await
            .context("could not parse json")
    }

    pub async fn open_channel(
        &self,
        target: NodeInfo,
        local_balance: u64,
        remote_balance: Option<u64>,
    ) -> Result<()> {
        self.post(
            "/api/channels",
            Some(ChannelParams {
                target: TargetInfo {
                    pubkey: target.pubkey.to_string(),
                    address: target.address.to_string(),
                },
                local_balance,
                remote_balance,
            }),
        )
        .await?;

        Ok(())
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.client
            .get(format!("{0}{path}", self.host))
            .send()
            .await
            .context("Could not send GET request to maker")?
            .error_for_status()
            .context("Maker did not return 200 OK")
    }

    async fn post<J>(&self, path: &str, json: Option<J>) -> Result<reqwest::Response>
    where
        J: Serialize,
    {
        let builder = self.client.post(format!("{0}{path}", self.host));

        let builder = match json {
            Some(ref json) => builder.json(json),
            None => builder,
        };

        builder
            .json(&json)
            .send()
            .await
            .context("Could not send POST request to maker")?
            .error_for_status()
            .context("Maker did not return 200 OK")
    }

    pub async fn create_invoice(&self, amount: Option<u64>) -> Result<Bolt11Invoice> {
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

    pub async fn broadcast_node_announcement(&self) -> Result<reqwest::Response> {
        let no_json: Option<()> = None;
        let status = self
            .post("/api/broadcast_announcement", no_json)
            .await?
            .error_for_status()?;
        Ok(status)
    }
}
