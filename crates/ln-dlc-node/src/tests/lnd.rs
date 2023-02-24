use crate::node::Node;
use crate::tests;
use crate::tests::bitcoind;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use lightning_invoice::Invoice;
use local_ip_address::local_ip;
use reqwest::Response;
use serde::Deserialize;
use std::time::Duration;
use tests::FAUCET_ORIGIN;

pub struct LndNode {}

#[derive(Deserialize, Debug)]
struct LndResponse {
    address: String,
}

impl LndNode {
    pub fn new() -> LndNode {
        LndNode {}
    }

    /// Funds the lnd onchain wallet.
    pub async fn fund(&self, amount: bitcoin::Amount) -> Result<()> {
        let response = self.get("lnd/v1/newaddress").await?;
        let response: LndResponse = response.json().await.unwrap();

        bitcoind::fund(response.address, amount).await?;
        bitcoind::mine(1).await?;

        // to wait for lnd to sync
        tokio::time::sleep(Duration::from_secs(5)).await;

        Ok(())
    }

    /// Instructs lnd to open a public channel with the target node.
    /// 1. Connect to the target node.
    /// 2. Open channel to the target node.
    /// 3. Wait for the channel to become usable on the target node. Note, this logic assumes that
    /// there is no other channel.
    pub async fn open_channel(&self, target: &Node, amount: bitcoin::Amount) -> Result<()> {
        let port = target.info.address.port();
        let ip_address = local_ip()?;
        let host = format!("{ip_address}:{port}");
        tracing::info!("Connecting lnd to {host}");
        self.post(
            "lnd/v1/peers",
            format!(
                r#"{{"addr": {{ "pubkey": "{}", "host": "{host}" }}, "perm":false }}"]"#,
                target.info.pubkey
            ),
        )
        .await?;

        tokio::time::sleep(Duration::from_secs(5)).await;

        tracing::info!("Opening channel to {} with {amount}", target.info);
        self.post(
            "lnd/v1/channels",
            format!(
                r#"{{"node_pubkey_string":"{}","local_funding_amount":"{}", "min_confs":1 }}"#,
                target.info.pubkey,
                amount.to_sat()
            ),
        )
        .await?;

        bitcoind::mine(1).await?;
        target.sync();

        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                // todo: it would be nicer if this logic would look for a channel open with the lnd
                // node.
                if target
                    .channel_manager
                    .list_usable_channels()
                    .first()
                    .is_some()
                {
                    break;
                }

                target.sync();

                tracing::debug!("Waiting for channel to be usable");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .map_err(|e| anyhow!(e))?;

        // todo: fetch channel status from lnd api instead of timeout.
        // wait for lnd to process the channel opening.
        tokio::time::sleep(Duration::from_secs(35)).await;
        Ok(())
    }

    /// Instructs lnd to send a payment for the given invoice
    pub async fn send_payment(&self, invoice: Invoice) -> Result<Response> {
        self.post(
            "lnd/v1/channels/transactions",
            format!(r#"{{"payment_request": "{invoice}"}}"#),
        )
        .await
    }

    async fn post(&self, path: &str, body: String) -> Result<Response> {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{FAUCET_ORIGIN}/{path}"))
            .body(body)
            .send()
            .await?;
        if !response.status().is_success() {
            bail!(response.text().await?)
        }

        Ok(response)
    }

    async fn get(&self, path: &str) -> Result<Response> {
        let client = reqwest::Client::new();
        let response = client.get(format!("{FAUCET_ORIGIN}/{path}")).send().await?;
        if !response.status().is_success() {
            bail!(response.text().await?)
        }
        Ok(response)
    }
}
