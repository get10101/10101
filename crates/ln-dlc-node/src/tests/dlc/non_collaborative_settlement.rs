use tokio::task::spawn_blocking;

use crate::tests::bitcoind::mine;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::init_tracing;

#[tokio::test(flavor = "multi_thread")]
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

    let coordinator = std::sync::Arc::new(coordinator);
    spawn_blocking({
        let coordinator = coordinator.clone();
        move || {
            coordinator
                .finalize_force_close_ln_dlc_channel(channel_details.channel_id)
                .unwrap()
        }
    })
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
    coordinator.dlc_manager.periodic_check().unwrap();

    // Confirm CET
    mine(1).await.unwrap();

    coordinator.wallet().sync().await.unwrap();

    let coordinator_on_chain_balance_after_force_close =
        coordinator.get_on_chain_balance().await.unwrap().confirmed;

    // Given that we have dynamic transaction fees based on the state of the regtest mempool, it's
    // less error-prone to choose a conservative lower bound on the funds we expect the coordinator
    // to get after force-closing the LN-DLC channel
    let coordinator_on_chain_balance_after_force_close_expected_min = 245_000;

    assert!(
        coordinator_on_chain_balance_after_force_close
            >= coordinator_on_chain_balance_after_force_close_expected_min
    );
}
