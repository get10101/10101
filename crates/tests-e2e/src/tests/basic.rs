use crate::app::run_app;
use crate::bitcoind::Bitcoind;
use crate::coordinator::Coordinator;
use crate::fund::fund_app_with_faucet;
use crate::http::init_reqwest;
use crate::logger::init_tracing;
use anyhow::Result;
use bitcoin::Amount;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_funded_with_lnd_faucet() -> Result<()> {
    init_tracing();

    let client = init_reqwest();
    let coordinator = Coordinator::new_local(client.clone());
    assert!(coordinator.is_running().await);

    // ensure coordinator has a free UTXO available
    let address = coordinator.get_new_address().await.unwrap();
    let bitcoind = Bitcoind::new_local(client.clone());
    bitcoind
        .send_to_address(&address, Amount::ONE_BTC)
        .await
        .unwrap();
    bitcoind.mine(1).await.unwrap();
    coordinator.sync_wallet().await.unwrap();

    let app = run_app(None).await;

    // Unfunded wallet should be empty
    assert_eq!(app.rx.wallet_info().unwrap().balances.lightning, 0);

    // open channel fees should be 11_000 sats (1%)
    let fund_amount = 1_100_000;
    fund_app_with_faucet(&app, &client, fund_amount).await?;

    Ok(())
}
