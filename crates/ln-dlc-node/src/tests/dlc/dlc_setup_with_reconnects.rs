use crate::node::dlc_channel::process_pending_dlc_actions;
use crate::node::Node;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::Context;
use bitcoin::Amount;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
#[ignore]
async fn reconnecting_during_dlc_channel_setup() {
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
        .unwrap()
        .clone();

    // Act

    let oracle_pk = app.oracle_pk();
    let contract_input = dummy_contract_input(20_000, 20_000, oracle_pk);

    app.propose_dlc_channel(channel_details.clone(), contract_input)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    app.reconnect(coordinator.info).await.unwrap();

    let sub_channel = wait_until(Duration::from_secs(30), || async {
        let sub_channels = coordinator
            .dlc_manager
            .get_store()
            .get_offered_sub_channels()?;

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

    // This is the point of this test: to verify that reconnecting during DLC channel setup can be
    // fixed by processing pending DLC actions
    app.reconnect(coordinator.info).await.unwrap();

    // Instruct coordinator to re-send the accept message
    process_pending_dlc_actions(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
    )
    .await
    .unwrap();

    // Process the coordinator's accept message _and_ send the confirm message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages().unwrap();

    // Process the confirm message _and_ send the finalize message
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    // Process the finalize message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages().unwrap();

    // Assert

    let channel = app
        .list_channels()
        .into_iter()
        .find(|channel| channel.channel_id == channel_details.channel_id);

    assert!(channel.is_some());

    let sub_channel_coordinator = coordinator
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .unwrap();

    matches!(sub_channel_coordinator.state, SubChannelState::Signed(_));

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .unwrap();

    matches!(sub_channel_app.state, SubChannelState::Signed(_));
}
