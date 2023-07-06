use anyhow::Result;
use bitcoin::{Address, Amount};
use std::str::FromStr;
use tests_e2e::app::run_app;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::fund::fund_app_with_faucet;
use tests_e2e::http::init_reqwest;
use tests_e2e::logger::init_tracing;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_funded_with_lnd_faucet() -> Result<()> {
    init_tracing();

    let client = init_reqwest();
    let coordinator = Coordinator::new_local(client.clone());
    assert!(coordinator.is_running().await);

    // ensure coordinator has a free UTXO available
    let address = coordinator.get_new_address().await.unwrap();
    let bitcoind = Bitcoind::new(client.clone());
    bitcoind
        .send_to_address(
            Address::from_str(address.as_str()).unwrap(),
            Amount::ONE_BTC,
        )
        .await
        .unwrap();
    bitcoind.mine(1).await.unwrap();
    coordinator.sync_wallet().await.unwrap();

    let app = run_app().await;

    // Unfunded wallet should be empty
    assert_eq!(app.rx.wallet_info().unwrap().balances.on_chain, 0);
    assert_eq!(app.rx.wallet_info().unwrap().balances.lightning, 0);

    let funded_amount = fund_app_with_faucet(&coordinator, &client, 50_000).await?;

    assert_eq!(app.rx.wallet_info().unwrap().balances.on_chain, 0);

    // FIXME: Asserting here on >= as this test run on the CI can't find a route when trying to pay
    // immediately after claiming a received payment.
    // See: https://github.com/get10101/10101/issues/883
    let ln_balance = app.rx.wallet_info().unwrap().balances.lightning;
    tracing::info!(%funded_amount, %ln_balance, "Successfully funded app with faucet");
    assert!(ln_balance >= funded_amount);
    Ok(())
}
