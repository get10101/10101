use crate::app::AppHandle;
use crate::wait_until;
use anyhow::bail;
use anyhow::Result;
use native::api;
use native::api::PaymentFlow;
use native::api::Status;
use native::api::WalletHistoryItem;
use native::api::WalletHistoryItemType;
use reqwest::Client;
use serde::Deserialize;
use std::cmp::max;
use tokio::task::spawn_blocking;

/// Instruct the LND faucet to pay an invoice generated with the purpose of opening a JIT channel
/// between the coordinator and an app.
pub async fn fund_app_with_faucet(
    app: &AppHandle,
    client: &Client,
    fund_amount: u64,
) -> Result<()> {
    spawn_blocking(move || api::register_beta("satoshi@vistomail.com".to_string()).unwrap())
        .await?;
    let fee_sats = max(fund_amount / 100, 10_000);
    let invoice =
        spawn_blocking(move || api::create_onboarding_invoice(1, fund_amount, fee_sats).unwrap())
            .await?;
    api::decode_destination(invoice.clone()).unwrap();

    pay_with_faucet(client, invoice).await?;

    // Ensure we sync the wallet info after funding
    spawn_blocking(move || api::refresh_wallet_info().unwrap()).await?;

    // Wait until the app has an outbound payment which should correspond to the channel-opening fee
    wait_until!(app
        .rx
        .wallet_info()
        .unwrap()
        .history
        .iter()
        .any(|item| matches!(
            item,
            WalletHistoryItem {
                wallet_type: WalletHistoryItemType::Lightning { .. },
                flow: PaymentFlow::Inbound,
                status: Status::Confirmed,
                ..
            }
        )));

    tracing::info!(%fund_amount, %fee_sats, "Successfully funded app with faucet");
    assert_eq!(
        app.rx.wallet_info().unwrap().balances.off_chain,
        fund_amount - fee_sats
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
