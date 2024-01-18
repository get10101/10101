use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use reqwest::Client;

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
