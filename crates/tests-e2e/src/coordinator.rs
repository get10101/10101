use anyhow::Context;
use anyhow::Result;
use reqwest::Client;

/// A wrapper over the coordinator HTTP API
///
/// It does not aim to be complete, functionality will be added as needed
pub struct Coordinator {
    client: Client,
    host: String,
}

impl Coordinator {
    pub fn new(client: Client) -> Self {
        let host = "http://localhost:8000".to_string();
        Self { client, host }
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
