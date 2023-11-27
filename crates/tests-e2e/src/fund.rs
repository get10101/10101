use crate::app::AppHandle;
use crate::bitcoind::Bitcoind;
use crate::http::init_reqwest;
use crate::wait_until;
use anyhow::bail;
use anyhow::Result;
use bitcoin::Amount;
use ln_dlc_node::node::NodeInfo;
use local_ip_address::local_ip;
use native::api;
use native::api::PaymentFlow;
use native::api::Status;
use native::api::WalletHistoryItem;
use native::api::WalletHistoryItemType;
use reqwest::Client;
use reqwest::Response;
use serde::Deserialize;
use std::cmp::max;
use std::time::Duration;
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
        app.rx.wallet_info().unwrap().balances.lightning,
        fund_amount - fee_sats
    );

    Ok(())
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

/// Instructs lnd to open a public channel with the target node.
/// 1. Connect to the target node.
/// 2. Open channel to the target node.
pub async fn open_channel(
    node_info: &NodeInfo,
    amount: Amount,
    faucet: &str,
    bitcoind: &Bitcoind,
) -> Result<()> {
    // Hacky way of checking whether we need to patch the coordinator
    // address when running locally
    let host = if faucet.to_string().contains("localhost") {
        let port = node_info.address.port();
        let ip_address = local_ip()?;
        let host = format!("{ip_address}:{port}");
        tracing::info!("Running locally, patching host to {host}");
        host
    } else {
        node_info.address.to_string()
    };
    tracing::info!("Connecting lnd to {host}");
    let res = post_query(
        "lnd/v1/peers",
        format!(
            r#"{{"addr": {{ "pubkey": "{}", "host": "{host}" }}, "perm":false }}"]"#,
            node_info.pubkey
        ),
        faucet,
    )
    .await;

    tracing::debug!(?res, "Response after attempting to connect lnd to {host}");

    tokio::time::sleep(Duration::from_secs(5)).await;

    tracing::info!("Opening channel to {} with {amount}", node_info);
    post_query(
        "lnd/v1/channels",
        format!(
            r#"{{"node_pubkey_string":"{}","local_funding_amount":"{}", "min_confs":1 }}"#,
            node_info.pubkey,
            amount.to_sat()
        ),
        faucet,
    )
    .await?;

    bitcoind.mine(10).await?;

    tracing::info!("connected to channel");

    Ok(())
}

async fn post_query(path: &str, body: String, faucet: &str) -> Result<Response> {
    let faucet = faucet.to_string();
    let client = init_reqwest();
    let response = client
        .post(format!("{faucet}/{path}"))
        .body(body)
        .send()
        .await?;

    if !response.status().is_success() {
        bail!(response.text().await?)
    }
    Ok(response)
}
