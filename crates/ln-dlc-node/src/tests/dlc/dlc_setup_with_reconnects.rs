use crate::node::Node;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::anyhow;
use anyhow::Context;
use bitcoin::Amount;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn reconnecting_during_dlc_channel_setup_leads_to_ln_channel_closure() {
    init_tracing();

    // Arrange

    let app = Node::start_test_app("app").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();

    app.connect(coordinator.info).await.unwrap();

    coordinator
        .fund(Amount::from_sat(10_000_000))
        .await
        .unwrap();

    coordinator
        .open_channel(&app, 50_000, 50_000)
        .await
        .unwrap();
    let channel_details = app.channel_manager.list_usable_channels();
    let channel_details = channel_details
        .iter()
        .find(|c| c.counterparty.node_id == coordinator.info.pubkey)
        .context("No usable channels for app")
        .unwrap();

    // Act/Assert

    let oracle_pk = app.oracle_pk();
    let contract_input = dummy_contract_input(20_000, 20_000, oracle_pk);

    app.propose_dlc_channel(channel_details, &contract_input)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    app.reconnect(coordinator.info).await.unwrap();

    // Check if channel is still open
    app.list_channels()
        .iter()
        .find(|channel| channel.channel_id == channel_details.channel_id)
        .expect("LN channel to be open");

    let sub_channel = wait_until(Duration::from_secs(30), || async {
        let sub_channels = coordinator
            .dlc_manager
            .get_store()
            .get_offered_sub_channels()
            .map_err(|e| anyhow!(e.to_string()))?;

        let sub_channel = sub_channels
            .iter()
            .find(|sub_channel| sub_channel.counter_party == app.info.pubkey);

        Ok(sub_channel.cloned())
    })
    .await
    .unwrap();

    coordinator
        .accept_dlc_channel_offer(&sub_channel.channel_id)
        .unwrap();

    // This reconnect leads to the channel being force-closed. This issue is tracked here:
    // https://github.com/get10101/10101/issues/352
    app.reconnect(coordinator.info).await.unwrap();

    // Channel is missing due to bug
    let channel = app
        .list_channels()
        .into_iter()
        .find(|channel| channel.channel_id == channel_details.channel_id);

    // todo: adapt this assertion to expect some channel once https://github.com/get10101/10101/issues/352 is fixed.
    assert!(channel.is_none());
}
