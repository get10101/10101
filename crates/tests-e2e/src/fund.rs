use anyhow::Result;
use native::api;
use reqwest::Client;
use reqwest::Response;
use tokio::task::spawn_blocking;

/// Pay a lightning invoice using an LND faucet
pub async fn fund_app_with_faucet(client: &Client, funding_amount: u64) -> Result<()> {
    let invoice = spawn_blocking(move || {
        api::create_invoice_with_amount(funding_amount).expect("to succeed")
    })
    .await?;
    api::decode_invoice(invoice.clone()).expect("to decode invoice we created");

    pay_with_faucet(client, invoice).await?;

    // Ensure we sync the wallet info after funding
    spawn_blocking(move || api::refresh_wallet_info().expect("to succeed")).await?;
    Ok(())
}

async fn pay_with_faucet(client: &Client, invoice: String) -> Result<Response> {
    #[derive(serde::Serialize)]
    struct PayInvoice {
        payment_request: String,
    }

    let faucet = "http://localhost:8080";
    let response = client
        .post(format!("{faucet}/lnd/v1/channels/transactions"))
        .body(serde_json::to_string(&PayInvoice {
            payment_request: invoice,
        })?)
        .send()
        .await?
        .error_for_status()?;
    Ok(response)
}
