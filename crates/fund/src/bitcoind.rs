use anyhow::bail;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use reqwest::Client;
use reqwest::Response;
use serde::Deserialize;
use std::time::Duration;

/// A wrapper over the bitcoind HTTP API
///
/// It does not aim to be complete, functionality will be added as needed
pub struct Bitcoind {
    client: Client,
    host: String,
}

impl Bitcoind {
    pub fn new(client: Client, host: String) -> Self {
        Self { client, host }
    }

    pub fn new_local(client: Client) -> Self {
        let host = "http://localhost:8080/bitcoin".to_string();
        Self::new(client, host)
    }

    /// Instructs `bitcoind` to generate to address.
    pub async fn mine(&self, n: u16) -> Result<()> {
        #[derive(Deserialize, Debug)]
        struct BitcoindResponse {
            result: String,
        }

        let response: BitcoindResponse = self
            .client
            .post(&self.host)
            .body(r#"{"jsonrpc": "1.0", "method": "getnewaddress", "params": []}"#.to_string())
            .send()
            .await?
            .json()
            .await?;

        self.client
            .post(&self.host)
            .body(format!(
                r#"{{"jsonrpc": "1.0", "method": "generatetoaddress", "params": [{}, "{}"]}}"#,
                n, response.result
            ))
            .send()
            .await?;

        // For the mined blocks to be picked up by the subsequent wallet syncs
        tokio::time::sleep(Duration::from_secs(5)).await;

        Ok(())
    }

    /// An alias for send_to_address
    pub async fn fund(&self, address: &Address, amount: Amount) -> Result<Response> {
        self.send_to_address(address, amount).await
    }

    pub async fn send_to_address(&self, address: &Address, amount: Amount) -> Result<Response> {
        let response = self
            .client
            .post(&self.host)
            .body(format!(
                r#"{{"jsonrpc": "1.0", "method": "sendtoaddress", "params": ["{}", "{}", "", "", false, false, null, null, false, 1.0]}}"#,
                address,
                amount.to_btc(),
            ))
            .send()
            .await?;
        Ok(response)
    }

    pub async fn post(&self, endpoint: &str, body: Option<String>) -> Result<Response> {
        let mut builder = self.client.post(endpoint.to_string());
        if let Some(body) = body {
            builder = builder.body(body);
        }
        let response = builder.send().await?;

        if !response.status().is_success() {
            bail!(response.text().await?)
        }
        Ok(response)
    }
}
