use crate::tests;
use anyhow::bail;
use anyhow::Result;
use reqwest::Response;
use serde::Deserialize;
use std::time::Duration;
use tests::FAUCET_ORIGIN;

#[derive(Deserialize, Debug)]
struct BitcoindResponse {
    result: String,
}

pub async fn fund(address: String, amount: bitcoin::Amount) -> Result<Response> {
    query(format!(
        r#"{{"jsonrpc": "1.0", "method": "sendtoaddress", "params": ["{}", "{}"]}}"#,
        address,
        amount.to_btc()
    ))
    .await
}

/// Instructs `bitcoind` to generate to address.
pub async fn mine(n: u16) -> Result<()> {
    let response =
        query(r#"{"jsonrpc": "1.0", "method": "getnewaddress", "params": []}"#.to_string()).await?;
    let response: BitcoindResponse = response.json().await.unwrap();

    query(format!(
        r#"{{"jsonrpc": "1.0", "method": "generatetoaddress", "params": [{}, "{}"]}}"#,
        n, response.result
    ))
    .await?;
    // For the mined blocks to be picked up by the subsequent wallet
    // syncs
    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(())
}

async fn query(query: String) -> Result<Response> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{FAUCET_ORIGIN}/bitcoin"))
        .body(query)
        .send()
        .await?;

    if !response.status().is_success() {
        bail!(response.text().await?)
    }
    Ok(response)
}
