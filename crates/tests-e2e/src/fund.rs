use anyhow::bail;
use anyhow::Result;
use native::api;
use reqwest::Client;
use serde::Deserialize;
use tokio::task::spawn_blocking;

// TODO: Fetch these from the app
pub const FUNDING_TRANSACTION_FEES: u64 = 153;

/// Pay a lightning invoice using an LND faucet
///
/// Returns the funded amount (in satoshis)
pub async fn fund_app_with_faucet(client: &Client, funding_amount: u64) -> Result<u64> {
    let invoice = spawn_blocking(move || {
        api::create_invoice_with_amount(funding_amount).expect("to succeed")
    })
    .await?;
    api::decode_invoice(invoice.clone()).expect("to decode invoice we created");

    pay_with_faucet(client, invoice).await?;

    // Ensure we sync the wallet info after funding
    spawn_blocking(move || api::refresh_wallet_info().expect("to succeed")).await?;

    Ok(funding_amount - FUNDING_TRANSACTION_FEES)
}

pub async fn pay_with_faucet(client: &Client, invoice: String) -> Result<()> {
    tracing::info!("Paying invoice with faucet: {}", invoice);

    #[derive(serde::Serialize)]
    struct PayInvoice {
        payment_request: String,
    }
    #[derive(Deserialize, Debug)]
    struct FaucetResponse {
        payment_error: Option<PaymentError>,
    }
    #[derive(Deserialize, Debug)]
    enum PaymentError {
        #[serde(rename = "insufficient_balance")]
        InsufficientBalance,
        #[serde(rename = "no_route")]
        NoRoute,
        #[serde(rename = "")]
        NoError,
    }

    let faucet = "http://localhost:8080";
    let body = serde_json::to_string(&PayInvoice {
        payment_request: invoice,
    })?;
    let response: FaucetResponse = client
        .post(format!("{faucet}/lnd/v1/channels/transactions"))
        .body(body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(payment_error) = response.payment_error {
        match payment_error {
            PaymentError::InsufficientBalance => {
                bail!("Could not fund wallet due to insufficient balance in faucet");
            }
            PaymentError::NoRoute => {
                bail!("Could not fund wallet due to no route found from faucet to app");
            }
            PaymentError::NoError => {
                tracing::info!("Payment succeeded 🚀")
            }
        }
    }
    Ok(())
}
