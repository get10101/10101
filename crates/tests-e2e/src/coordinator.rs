use anyhow::Context;
use anyhow::Result;
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
        // We assume that if we can generate new address, the service is running
        self.get("/api/newaddress").await.is_ok()
    }

    pub async fn is_node_connected(&self, node_id: &str) -> Result<bool> {
        let result = self
            .get(&format!("/api/admin/is_connected/{node_id}"))
            .await?
            .status()
            .is_success();
        Ok(result)
    }

    pub async fn sync_wallet(&self) -> Result<bool> {
        let result = self
            .client
            .post(&format!("{}/api/admin/sync", self.host))
            .send()
            .await?
            .status()
            .is_success();
        Ok(result)
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

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.client
            .get(format!("{0}{path}", self.host))
            .send()
            .await
            .context("Could not send request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }
}
