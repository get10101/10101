use crate::node::sub_channel::sub_channel_manager_periodic_check;
use crate::node::Node;
use crate::tests::bitcoind::mine;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::init_tracing;
use bitcoin::Amount;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn force_close_ln_dlc_channel() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    let app_ln_balance = app_dlc_collateral * 2;
    let coordinator_ln_balance = coordinator_dlc_collateral * 2;

    let fund_amount = (app_ln_balance + coordinator_ln_balance) * 2;

    let (app, _running_app) = Node::start_test_app("app").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();

    app.connect(coordinator.info).await.unwrap();

    coordinator
        .fund(Amount::from_sat(fund_amount))
        .await
        .unwrap();

    let channel_details = coordinator
        .open_private_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await
        .unwrap();

    create_dlc_channel(
        &app,
        &coordinator,
        app_dlc_collateral,
        coordinator_dlc_collateral,
    )
    .await
    .unwrap();

    coordinator.sync_wallets().await.unwrap();
    app.sync_wallets().await.unwrap();

    // Act

    coordinator.force_close_channel(&channel_details).unwrap();

    // Need 288 confirmations on the split transaction to be able to publish the glue and buffer
    // transactions
    mine(288).await.unwrap();

    coordinator.sync_wallets().await.unwrap();
    app.sync_wallets().await.unwrap();

    // Ensure publication of the glue and buffer transactions (otherwise we need to wait for the
    // periodic task)
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
        &coordinator.peer_manager,
    )
    .await
    .unwrap();

    // Assert

    coordinator.sync_wallets().await.unwrap();
    app.sync_wallets().await.unwrap();

    // Mining 288 blocks ensures that we get:
    // - 144 required confirmations for the delayed output on the LN commitment transaction to be
    // spendable.
    // - 288 required confirmations for the CET to be published.
    mine(288).await.unwrap();

    coordinator.sync_wallets().await.unwrap();
    app.sync_wallets().await.unwrap();

    // Ensure publication of CET (otherwise we need to wait for the periodic task)
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
        &coordinator.peer_manager,
    )
    .await
    .unwrap();

    // Confirm CET
    mine(1).await.unwrap();
    tracing::info!("Mined 1 block");

    coordinator.sync_wallets().await.unwrap();
    tracing::info!("Coordinator synced on-chain");
    app.sync_wallets().await.unwrap();
    tracing::info!("App synced on-chain");

    let coordinator_on_chain_balance_after_force_close =
        coordinator.get_on_chain_balance().unwrap().confirmed;
    tracing::info!(balance = %coordinator_on_chain_balance_after_force_close, "Coordinator on-chain balance");
    let app_on_chain_balance_after_force_close = app.get_on_chain_balance().unwrap().confirmed;
    tracing::info!(balance = %app_on_chain_balance_after_force_close, "App on-chain balance");

    // Given that we have dynamic transaction fees based on the state of the regtest mempool, it's
    // less error-prone to choose a conservative lower bound on the expected funds after
    // force-closing the LN-DLC channel
    let coordinator_on_chain_balance_after_force_close_expected_min = 245_000;
    let app_on_chain_balance_after_force_close_expected_min = 45_000;

    assert!(
        coordinator_on_chain_balance_after_force_close
            >= coordinator_on_chain_balance_after_force_close_expected_min
    );

    assert!(
        app_on_chain_balance_after_force_close
            >= app_on_chain_balance_after_force_close_expected_min
    );
}
