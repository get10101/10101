use anyhow::Result;
use native::api;
use tests_e2e::app::run_app;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::fund::fund_app_with_faucet;
use tests_e2e::http::init_reqwest;
use tests_e2e::tracing::init_tracing;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_funded_with_lnd_faucet() -> Result<()> {
    init_tracing();

    let client = init_reqwest();
    let coordinator = Coordinator::new_local(client.clone());
    assert!(coordinator.is_running().await);

    let bitcoind = Bitcoind::new(client.clone());

    let app = run_app().await;

    // this is just to showcase we can retrieve value from a SyncReturn
    let node_id: String = api::get_node_id().0;
    tracing::info!("Node ID: {}", node_id);

    // Unfunded wallet should be empty
    assert_eq!(app.rx.wallet_info().unwrap().balances.on_chain, 0);
    assert_eq!(app.rx.wallet_info().unwrap().balances.lightning, 0);

    // TODO: Remove this when fixed. We mine a block before funding the app to ensure that all
    // outputs are spendable. This is necessary as the test might otherwise fail due to missing
    // or unspendable output when broadcasting the funding transaction.
    bitcoind.mine(1).await?;
    coordinator.sync_wallet().await?;

    let funding_amount = 50_000;
    let funding_transaction_fees = 153;
    fund_app_with_faucet(&client, funding_amount).await?;

    // TODO: Remove this when fixed. We mine a block before funding the app to ensure that all
    // outputs are spendable. This is necessary as the test might otherwise fail due to missing
    // or unspendable output when broadcasting the funding transaction.
    bitcoind.mine(1).await?;
    coordinator.sync_wallet().await?;

    assert_eq!(app.rx.wallet_info().unwrap().balances.on_chain, 0);

    // TODO: Asserting here on >= as this test run on the CI can't find a route when trying to pay
    // immediately after claiming a received payment.
    assert!(
        app.rx.wallet_info().unwrap().balances.lightning
            >= funding_amount - funding_transaction_fees
    );
    Ok(())
}
