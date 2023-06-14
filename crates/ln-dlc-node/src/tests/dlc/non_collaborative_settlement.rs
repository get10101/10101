use crate::node::dlc_channel::dlc_manager_periodic_check;
use crate::node::dlc_channel::sub_channel_manager_periodic_check;
use crate::tests::bitcoind::mine;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::init_tracing;

#[tokio::test]
#[ignore]
async fn force_close_ln_dlc_channel() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    let DlcChannelCreated {
        coordinator,
        app,
        channel_details,
        ..
    } = create_dlc_channel(app_dlc_collateral, coordinator_dlc_collateral)
        .await
        .unwrap();

    coordinator.wallet().sync().await.unwrap();
    app.wallet().sync().await.unwrap();

    // Act

    coordinator.force_close_channel(&channel_details).unwrap();

    // Need 288 confirmations on the split transaction to be able to publish the glue and buffer
    // transactions
    mine(288).await.unwrap();

    coordinator.wallet().sync().await.unwrap();
    app.wallet().sync().await.unwrap();

    // Ensure publication of the glue and buffer transactions (otherwise we need to wait for the
    // periodic task)
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
    )
    .await
    .unwrap();

    // Assert

    coordinator.wallet().sync().await.unwrap();
    app.wallet().sync().await.unwrap();

    // Mining 288 blocks ensures that we get:
    // - 144 required confirmations for the delayed output on the LN commitment transaction to be
    // spendable.
    // - 288 required confirmations for the CET to be published.
    mine(288).await.unwrap();

    coordinator.wallet().sync().await.unwrap();
    app.wallet().sync().await.unwrap();

    // Ensure publication of CET (otherwise we need to wait for the periodic task)
    dlc_manager_periodic_check(coordinator.dlc_manager.clone())
        .await
        .unwrap();

    // Confirm CET
    mine(1).await.unwrap();

    coordinator.wallet().sync().await.unwrap();
    app.wallet().sync().await.unwrap();

    let coordinator_on_chain_balance_after_force_close =
        coordinator.get_on_chain_balance().unwrap().confirmed;
    let app_on_chain_balance_after_force_close = app.get_on_chain_balance().unwrap().confirmed;

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
