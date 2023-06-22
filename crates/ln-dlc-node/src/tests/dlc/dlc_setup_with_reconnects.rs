use crate::node::dlc_channel::sub_channel_manager_periodic_check;
use crate::node::Node;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use bitcoin::Amount;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
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

    // Wait for the `Confirm` message to be delivered
    tokio::time::sleep(Duration::from_secs(2)).await;

    app.reconnect(coordinator_info).await.unwrap();

    // Wait for the peers to reconnect and get the `ChannelReestablish` event. During the reconnect
    // the coordinator will return from `Accepted` to the `Offered` state.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // The coordinator handles `ReAccept` action. We need this so that the coordinator advances its
    // state to `Accepted` again, so that it can process the app's old `Confirm` message
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
    )
    .await
    .unwrap();

    // tracing::info!("---> App processing second `Accept` <---");

    // tokio::time::sleep(Duration::from_secs(2)).await;

    // app.process_incoming_messages().unwrap();

    // tokio::time::sleep(Duration::from_secs(2)).await;

    tracing::info!("---> Coordinator processing app's `Confirm` message now <---");

    // FIXME: Processing the SubChannelConfirm message here will result in the following error
    // Invalid state: Misuse error: Close : Got a revoke commitment secret which didn't correspond
    // to their current pubkey
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

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_lose_connection_before_processing_subchannel_close_finalize() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 25_000;
    let coordinator_dlc_collateral = 50_000;

    let app_ln_balance = app_dlc_collateral * 2;
    let coordinator_ln_balance = coordinator_dlc_collateral * 2;

    let fund_amount = (app_ln_balance + coordinator_ln_balance) * 2;

    let app = Node::start_test_app("app").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();

    app.connect(coordinator.info).await.unwrap();

    coordinator
        .fund(Amount::from_sat(fund_amount))
        .await
        .unwrap();

    coordinator
        .open_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    create_dlc_channel(
        &coordinator,
        &app,
        coordinator_dlc_collateral,
        app_dlc_collateral,
    )
    .await
    .unwrap();

    let channel_details = coordinator
        .channel_manager
        .list_usable_channels()
        .iter()
        .find(|c| c.counterparty.node_id == app.info.pubkey)
        .unwrap()
        .clone();

    coordinator
        .propose_dlc_channel_collaborative_settlement(
            channel_details.channel_id,
            app_dlc_collateral / 2,
        )
        .await
        .unwrap();

    // Process `CloseOffer`
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::CloseOffered,
    )
    .await
    .unwrap();

    app.accept_dlc_channel_collaborative_settlement(&sub_channel.channel_id)
        .unwrap();

    // Process `CloseAccept` and send `CloseConfirm`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::CloseConfirmed,
    )
    .await
    .unwrap();

    // Act

    // Process `CloseConfirm` and send `CloseFinalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    app.reconnect(coordinator.info).await.unwrap();

    coordinator.process_incoming_messages().unwrap();

    // Assert

    let state = coordinator
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .first()
        .unwrap()
        .state
        .clone();

    assert!(matches!(state, SubChannelState::OffChainClosed));
}
