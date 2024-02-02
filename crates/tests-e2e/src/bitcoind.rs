use anyhow::bail;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use reqwest::Client;
use reqwest::Response;
use serde::Deserialize;
use serde_json::json;
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
        let response: GetNewAddressResponse = self
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

    pub async fn send_multiple_utxos_to_address<F>(
        &self,
        address_fn: F,
        utxo_amount: Amount,
        n_utxos: u64,
    ) -> Result<()>
    where
        F: Fn() -> Address,
    {
        let total_amount = utxo_amount * n_utxos;

        let response: ListUnspentResponse = self
            .client
            .post(&self.host)
            .body(r#"{"jsonrpc": "1.0", "method": "listunspent", "params": []}"#)
            .send()
            .await?
            .json()
            .await?;

        let utxo = response
            .result
            .iter()
            // We try to find one UTXO that can cover the whole transaction. We could cover the
            // amount with multiple UTXOs too, but this is simpler and will probably succeed.
            .find(|utxo| utxo.spendable && utxo.amount >= total_amount)
            .expect("to find UTXO to cover multi-payment");

        let mut outputs = serde_json::value::Map::new();

        for _ in 0..n_utxos {
            let address = address_fn();
            outputs.insert(address.to_string(), json!(utxo_amount.to_btc()));
        }

        let create_raw_tx_request = json!(
            {
                "jsonrpc": "1.0",
                "method": "createrawtransaction",
                "params":
                [
                    [ {"txid": utxo.txid, "vout": utxo.vout} ],
                    outputs
                ]
            }
        );

        let create_raw_tx_response: CreateRawTransactionResponse = self
            .client
            .post(&self.host)
            .json(&create_raw_tx_request)
            .send()
            .await?
            .json()
            .await?;

        let sign_raw_tx_with_wallet_request = json!(
            {
                "jsonrpc": "1.0",
                "method": "signrawtransactionwithwallet",
                "params": [ create_raw_tx_response.result ]
            }
        );

        let sign_raw_tx_with_wallet_response: SignRawTransactionWithWalletResponse = self
            .client
            .post(&self.host)
            .json(&sign_raw_tx_with_wallet_request)
            .send()
            .await?
            .json()
            .await?;

        let send_raw_tx_request = json!(
            {
                "jsonrpc": "1.0",
                "method": "sendrawtransaction",
                "params": [ sign_raw_tx_with_wallet_response.result.hex, 0 ]
            }
        );

        let send_raw_tx_response: SendRawTransactionResponse = self
            .client
            .post(&self.host)
            .json(&send_raw_tx_request)
            .send()
            .await?
            .json()
            .await?;

        tracing::info!(
            txid = %send_raw_tx_response.result,
            %utxo_amount,
            %n_utxos,
            "Published multi-utxo transaction"
        );

        Ok(())
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

#[derive(Deserialize, Debug)]
struct GetNewAddressResponse {
    result: String,
}

#[derive(Deserialize, Debug)]
struct ListUnspentResponse {
    result: Vec<Utxo>,
}

#[derive(Deserialize, Debug)]
struct Utxo {
    txid: String,
    vout: usize,
    #[serde(with = "bitcoin::util::amount::serde::as_btc")]
    amount: Amount,
    spendable: bool,
}

#[derive(Deserialize, Debug)]
struct CreateRawTransactionResponse {
    result: String,
}

#[derive(Deserialize, Debug)]
struct SignRawTransactionWithWalletResponse {
    result: SignRawTransactionWithWalletResponseBody,
}

#[derive(Deserialize, Debug)]
struct SignRawTransactionWithWalletResponseBody {
    hex: String,
}

#[derive(Deserialize, Debug)]
struct SendRawTransactionResponse {
    result: String,
}
