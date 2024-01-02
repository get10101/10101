use crate::node::Node;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use bitcoin::Amount;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn vanilla_dlc_channel() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 10_000;
    let coordinator_dlc_collateral = 10_000;

    let (app, _running_app) = Node::start_test_app("app").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();

    app.connect(coordinator.info).await.unwrap();

    // Choosing large fund amounts compared to the DLC collateral to ensure that we have one input
    // per party. In the end, it doesn't seem to matter though.

    app.fund(Amount::from_sat(10_000_000)).await.unwrap();

    coordinator
        .fund(Amount::from_sat(10_000_000))
        .await
        .unwrap();

    // Act

    let oracle_pk = *coordinator.oracle_pk().first().unwrap();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    coordinator
        .propose_dlc_channel(contract_input, app.info.pubkey)
        .await
        .unwrap();

    let offered_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app.dlc_manager.get_store().get_offered_channels()?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();

    app.accept_dlc_channel_offer(&offered_channel.temporary_channel_id)
        .unwrap();

    let _coordinator_signed_channel = wait_until(Duration::from_secs(30), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(None)?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();

    let _app_signed_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app.dlc_manager.get_store().get_signed_channels(None)?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();
}
