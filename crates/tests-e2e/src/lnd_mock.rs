use anyhow::Context;
use anyhow::Result;
use reqwest::Client;
use serde::Serialize;

/// A wrapper over the lnd mock HTTP API.
///
/// It does not aim to be complete, functionality will be added as needed.
pub struct LndMock {
    client: Client,
    host: String,
}

impl LndMock {
    pub fn new(client: Client, host: &str) -> Self {
        Self {
            client,
            host: host.to_string(),
        }
    }

    pub fn new_local(client: Client) -> Self {
        Self::new(client, "http://localhost:18080")
    }

    pub async fn pay_invoice(&self) -> Result<reqwest::Response> {
        self.post::<()>("/pay_invoice", None).await
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
            .context("Could not send POST request to lnd mock")?
            .error_for_status()
            .context("Lnd mock did not return 200 OK")
    }
}
