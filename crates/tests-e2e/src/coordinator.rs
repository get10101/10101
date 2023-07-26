use anyhow::Context;
use anyhow::Result;
use coordinator::routes::InvoiceParams;
use ln_dlc_node::node::NodeInfo;
use reqwest::Client;

/// A wrapper over the coordinator HTTP API
///
/// It does not aim to be complete, functionality will be added as needed
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

    /// Check whether the coordinator is running
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

    pub async fn create_invoice(&self, amount: Option<u64>) -> Result<String> {
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
            .await?;
        Ok(invoice)
    }

    // TODO: Introduce strong type
    pub async fn get_new_address(&self) -> Result<String> {
        Ok(self
            .get("/api/newaddress")
            .await?
            .text()
            .await?
            .strip_prefix('"')
            .to_owned()
            .expect("prefix")
            .strip_suffix('"')
            .expect("suffix")
            .to_owned())
    }

    // TODO: Introduce strong type
    pub async fn get_balance(&self) -> Result<String> {
        Ok(self.get("/api/admin/balance").await?.text().await?)
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
