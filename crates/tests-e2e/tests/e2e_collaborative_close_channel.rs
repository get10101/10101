#![allow(clippy::unwrap_used)]

use native::api;
use tests_e2e::app::refresh_wallet_info;
use tests_e2e::setup;
use tests_e2e::setup::dummy_order;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_open_and_collab_close_channel() {
    // Setup
    let test = setup::TestSetup::new_with_open_position().await;

    let app_off_chain_balance = test
        .app
        .rx
        .wallet_info()
        .unwrap()
        .balances
        .off_chain
        .unwrap();
    tracing::info!(%app_off_chain_balance, "Opened position");

    let closing_order = {
        let mut order = dummy_order();
        order.direction = api::Direction::Short;
        order
    };

    tracing::info!("Closing first position");

    spawn_blocking(move || api::submit_order(closing_order).unwrap())
        .await
        .unwrap();

    wait_until!(test.app.rx.position_close().is_some());

    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let app_on_chain_balance = test.app.rx.wallet_info().unwrap().balances.on_chain;
    let app_off_chain_balance = test
        .app
        .rx
        .wallet_info()
        .unwrap()
        .balances
        .off_chain
        .unwrap();
    tracing::info!(%app_off_chain_balance, "Closed first position");

    // Act
    spawn_blocking(move || api::close_channel().unwrap())
        .await
        .unwrap();

    // wait until there is no balance off-chain anymore
    wait_until!({
        test.bitcoind.mine(1).await.unwrap();
        refresh_wallet_info();
        let app_balance = test.app.rx.wallet_info().unwrap().balances;
        tracing::info!(
            off_chain = app_balance.off_chain,
            on_chain = app_balance.on_chain,
            "Balance while waiting"
        );
        app_balance.off_chain.unwrap() == 0
    });

    // Assert

    let wallet_info = test.app.rx.wallet_info().unwrap();
    assert_eq!(
        wallet_info.balances.on_chain,
        app_on_chain_balance + app_off_chain_balance
    );

    // TODO: Assert that the coordinator's balance
}
