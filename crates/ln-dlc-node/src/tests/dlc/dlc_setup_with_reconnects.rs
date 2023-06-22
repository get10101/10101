use crate::node::dlc_channel::sub_channel_manager_periodic_check;
use crate::node::Node;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use bitcoin::Amount;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn reconnecting_during_dlc_channel_setup() {
    init_tracing();

    // Arrange

    let app = Arc::new(Node::start_test_app("app").unwrap());
    let coordinator = Arc::new(Node::start_test_coordinator("coordinator").unwrap());

    let coordinator_info = coordinator.info;

    app.connect(coordinator_info).await.unwrap();

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
        .find(|c| c.counterparty.node_id == coordinator_info.pubkey)
        .context("No usable channels for app")
        .unwrap()
        .clone();

    // Act

    let oracle_pk = app.oracle_pk();
    let contract_input = dummy_contract_input(20_000, 20_000, oracle_pk);

    app.propose_dlc_channel(channel_details.clone(), contract_input)
        .await
        .unwrap();

    // Process the app's `Offer`
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Offered,
    )
    .await
    .unwrap();

    app.disconnect(coordinator.info);
    // we need to wait a few seconds for the disconnect to get updated to the channel_state.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // assert that the accept dlc_channel_offer fails if the peers are disconnected (do not
    // panic as in https://github.com/get10101/10101/issues/760).
    assert!(coordinator
        .accept_dlc_channel_offer(&sub_channel.channel_id)
        .is_err());

    app.connect(coordinator.info).await.unwrap();

    coordinator
        .accept_dlc_channel_offer(&sub_channel.channel_id)
        .unwrap();

    // This is the point of this test: to verify that reconnecting during DLC channel setup can be
    // fixed by processing pending DLC actions
    app.reconnect(coordinator.info).await.unwrap();

    // Instruct coordinator to re-send the accept message
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
    )
    .await
    .unwrap();

    // Process the coordinator's `Accept` and send `Confirm`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Confirmed,
    )
    .await
    .unwrap();

    // TODO: I have absolutely no idea why reconnecting sequentially does not result into the same
    // error as when we are reconnecting in an async task!
    // app.reconnect(coordinator_info).await.unwrap();

    // TODO: Check why the reconnect has to happen in a dedicated task!
    tokio::spawn({
        let app = app.clone();
        let info = coordinator.info;
        async move {
            app.reconnect(info).await.unwrap();
        }
    });

    // Wait for the peer to get actually connect and the channel reestablish event to finish.
    // During the reconnect the coordinator will return from `Accepted` to the `Offer` state

    // After 5 seconds the reaccept message is not yet automatically send through the pending
    // actions, but can be enforced by manually calling the function
    // tokio::time::sleep(Duration::from_secs(5)).await;

    // reaccept the dlc channel offer
    // coordinator
    //     .accept_dlc_channel_offer(&sub_channel.channel_id)
    //     .unwrap();

    // Alternatively, we can wait for about 25 seconds so that the reaccept message gets
    // automatically sent.
    tokio::time::sleep(Duration::from_secs(25)).await;

    // Process the app's `Confirm` and send `Finalize`
    // FIXME: Processing the SubChannelConfirm message here will result in the following error
    // Invalid state: Misuse error: Close : Got a revoke commitment secret which didn't correspond
    // to their current pubkey
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Accepted,
    )
    .await
    .unwrap();

    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();

    assert!(app
        .list_channels()
        .iter()
        .any(|channel| channel.channel_id == channel_details.channel_id));
}
