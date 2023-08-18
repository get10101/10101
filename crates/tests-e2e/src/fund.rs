use crate::app::AppHandle;
use crate::wait_until;
use anyhow::bail;
use anyhow::Result;
use native::api;
use native::api::PaymentFlow;
use native::api::Status;
use native::api::WalletHistoryItem;
use reqwest::Client;
use serde::Deserialize;
use tokio::task::spawn_blocking;

/// Instruct the LND faucet to pay an invoice generated with the purpose of opening a JIT channel
/// between the coordinator and an app.
pub async fn fund_app_with_faucet(
    app: &AppHandle,
    client: &Client,
    fund_amount: u64,
) -> Result<()> {
    let invoice =
        spawn_blocking(move || api::create_invoice_with_amount(fund_amount).expect("to succeed"))
            .await?;
    api::decode_invoice(invoice.clone()).expect("to decode invoice we created");

    pay_with_faucet(client, invoice).await?;

    // Ensure we sync the wallet info after funding
    spawn_blocking(move || api::refresh_wallet_info().expect("to succeed")).await?;

    // Wait until the app has an outbound payment which should correspond to the channel-opening fee
    wait_until!(app
        .rx
        .wallet_info()
        .expect("to have wallet info")
        .history
        .iter()
        .any(|item| matches!(
            item,
            WalletHistoryItem {
                flow: PaymentFlow::Outbound,
                status: Status::Confirmed,
                ..
            }
        )));

    let order_matching_fee = app
        .rx
        .wallet_info()
        .expect("to have wallet info")
        .history
        .iter()
        .find_map(|item| match item {
            WalletHistoryItem {
                flow: PaymentFlow::Outbound,
                status: Status::Confirmed,
                amount_sats,
                ..
            } => Some(amount_sats),
            _ => None,
        })
        .copied()
        .expect("to have an order-matching fee");

    tracing::info!(%fund_amount, %order_matching_fee, "Successfully funded app with faucet");
    assert_eq!(
        app.rx
            .wallet_info()
            .expect("to have wallet info")
            .balances
            .lightning,
        fund_amount - order_matching_fee
    );

    Ok(())
}

async fn pay_with_faucet(client: &Client, invoice: String) -> Result<()> {
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
