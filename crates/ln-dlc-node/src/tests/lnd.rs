use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests;
use crate::tests::bitcoind;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use lightning_invoice::Invoice;
use local_ip_address::local_ip;
use reqwest::Response;
use serde::Deserialize;
use std::time::Duration;
use tests::FAUCET_ORIGIN;

pub struct LndNode {}

impl LndNode {
    pub fn new() -> LndNode {
        LndNode {}
    }

    /// Funds the lnd onchain wallet.
    pub async fn fund(&self, amount: bitcoin::Amount) -> Result<()> {
        #[derive(Deserialize, Debug)]
        struct NewAddressResponse {
            address: String,
        }

        let response = self.get("lnd/v1/newaddress").await?;
        let response: NewAddressResponse = response.json().await.unwrap();

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
    pub async fn open_channel(
        &self,
        target: &Node<PaymentMap>,
        amount: bitcoin::Amount,
    ) -> Result<()> {
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
        target.wallet().sync().await.unwrap();

        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                if self.is_channel_active(target.info.pubkey).await? {
                    break;
                }

                target.wallet().sync().await.unwrap();

                tracing::debug!("Waiting for channel to be usable");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            anyhow::Ok(())
        })
        .await??;

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

    async fn is_channel_active(&self, remote_pubkey: PublicKey) -> Result<bool> {
        #[derive(Debug, Deserialize)]
        struct ListChannelsResponse {
            channels: Vec<LndChannel>,
        }

        #[derive(Debug, Deserialize)]
        struct LndChannel {
            remote_pubkey: String,
            active: bool,
        }

        let response = self.get("lnd/v1/channels").await?;
        let channels: ListChannelsResponse = response.json().await.unwrap();

        Ok(channels
            .channels
            .iter()
            .any(|c| c.remote_pubkey == remote_pubkey.to_string() && c.active))
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
