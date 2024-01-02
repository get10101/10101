use crate::node::sub_channel::sub_channel_manager_periodic_check;
use crate::node::Node;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_for_n_usable_channels;
use crate::tests::wait_until;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use bitcoin::Amount;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn reconnecting_during_dlc_channel_setup() {
    init_tracing();

    // Arrange

    let (app, _running_app) = Node::start_test_app("app").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();

    let coordinator_info = coordinator.info;

    app.connect(coordinator_info).await.unwrap();

    coordinator
        .fund(Amount::from_sat(10_000_000))
        .await
        .unwrap();

    coordinator
        .open_private_channel(&app, 50_000, 50_000)
        .await
        .unwrap();

    wait_for_n_usable_channels(1, &coordinator).await.unwrap();
    let channel_details = app
        .channel_manager
        .list_usable_channels()
        .iter()
        .find(|c| c.counterparty.node_id == coordinator_info.pubkey)
        .context("No usable channels for app")
        .unwrap()
        .clone();

    // Act

    let oracle_pk = *app.oracle_pk().first().unwrap();
    let contract_input = dummy_contract_input(20_000, 20_000, oracle_pk);

    app.propose_sub_channel(channel_details.clone(), contract_input)
        .await
        .unwrap();

    // Process the app's `Offer`.
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Offered,
    )
    .await
    .unwrap();

    app.disconnect(coordinator.info);

    // We need to wait for the channel to not be usable after the disconnect.
    wait_until(Duration::from_secs(5), || async {
        Ok(coordinator.list_usable_channels().is_empty().then_some(()))
    })
    .await
    .unwrap();

    // Assert that `accept_dlc_channel_offer` fails if the peer is disconnected (do not panic as in
    // https://github.com/get10101/10101/issues/760).
    assert!(coordinator
        .accept_sub_channel_offer(&sub_channel.channel_id)
        .is_err());

    app.connect(coordinator.info).await.unwrap();

    coordinator
        .accept_sub_channel_offer(&sub_channel.channel_id)
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

    // Wait for the `Confirm` message to be delivered.
    wait_until(Duration::from_secs(5), || async {
        Ok(coordinator
            .dlc_message_handler
            .has_pending_messages_to_process()
            .then_some(()))
    })
    .await
    .unwrap();

    app.reconnect(coordinator_info).await.unwrap();

    // Process the app's first `Confirm` message. This should fail because of an invalid commitment
    // transaction number.
    coordinator
        .process_incoming_messages()
        .expect_err("should have invalid commitment transaction number");

    // Create second `Accept` message from pending `ReAccept` action.
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
        &coordinator.peer_manager,
    )
    .await
    .unwrap();

    // Process the coordinator's second `Accept` and send `Confirm`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Confirmed,
    )
    .await
    .unwrap();

    // Process the app's second `Confirm` message and send `Finalize`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Finalized,
    )
    .await
    .unwrap();

    // Process the coordinator's `Finalize` message and send `Revoke`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();

    // Process the app's `Revoke` message.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();

    let coordinator_settlement_amount = 12_500;
    app.propose_sub_channel_collaborative_settlement(
        channel_details.channel_id,
        coordinator_settlement_amount,
    )
    .await
    .unwrap();

    // Process the app's `CloseOffer`.
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::CloseOffered,
    )
    .await
    .unwrap();

    coordinator
        .accept_sub_channel_collaborative_settlement(&sub_channel.channel_id)
        .unwrap();

    // Process the coordinator's `CloseAccept` and send `CloseConfirm`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::CloseConfirmed,
    )
    .await
    .unwrap();

    // Wait for the `CloseConfirm` message to be delivered.
    wait_until(Duration::from_secs(5), || async {
        Ok(coordinator
            .dlc_message_handler
            .has_pending_messages_to_process()
            .then_some(()))
    })
    .await
    .unwrap();

    app.reconnect(coordinator_info).await.unwrap();

    // Process the app's first `CloseConfirm` message. This should fail because of an invalid
    // commitment transaction number.
    coordinator
        .process_incoming_messages()
        .expect_err("should have invalid commitment transaction number");

    // Create second `CloseAccept` message from pending `ReAcceptCloseOffer` action.
    sub_channel_manager_periodic_check(
        coordinator.sub_channel_manager.clone(),
        &coordinator.dlc_message_handler,
        &coordinator.peer_manager,
    )
    .await
    .unwrap();

    // Process the coordinator's second `CloseAccept` and send `CloseConfirm`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::CloseConfirmed,
    )
    .await
    .unwrap();

    // Process the app's `CloseConfirm` and send `CloseFinalize`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await
    .unwrap();

    // Process the coordinator's `CloseFinalize`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await
    .unwrap();
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

    let (app, _running_app) = Node::start_test_app("app").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();

    app.connect(coordinator.info).await.unwrap();

    coordinator
        .fund(Amount::from_sat(fund_amount))
        .await
        .unwrap();

    coordinator
        .open_private_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await
        .unwrap();

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
        .propose_sub_channel_collaborative_settlement(
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

    app.accept_sub_channel_collaborative_settlement(&sub_channel.channel_id)
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

    wait_until(Duration::from_secs(5), || async {
        Ok(coordinator
            .dlc_message_handler
            .has_pending_messages_to_process()
            .then_some(()))
    })
    .await
    .unwrap();

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

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_lose_connection_before_processing_subchannel_accept() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 25_000;
    let coordinator_dlc_collateral = 50_000;

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

    coordinator
        .open_private_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await
        .unwrap();

    let oracle_pk = *app.oracle_pk().first().unwrap();
    let contract_input =
        dummy_contract_input(coordinator_dlc_collateral, app_dlc_collateral, oracle_pk);

    wait_for_n_usable_channels(1, &coordinator).await.unwrap();
    let channel_details = coordinator
        .channel_manager
        .list_usable_channels()
        .iter()
        .find(|c| c.counterparty.node_id == app.info.pubkey)
        .context("Could not find usable channel with peer")
        .unwrap()
        .clone();

    coordinator
        .propose_sub_channel(channel_details.clone(), contract_input)
        .await
        .unwrap();

    // Process the coordinator's `Offer`.
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Offered,
    )
    .await
    .unwrap();

    app.accept_sub_channel_offer(&sub_channel.channel_id)
        .unwrap();

    // Give time to deliver the `Accept` message to the coordinator.
    wait_until(Duration::from_secs(5), || async {
        Ok(coordinator
            .dlc_message_handler
            .has_pending_messages_to_process()
            .then_some(()))
    })
    .await
    .unwrap();

    // Lose the connection, triggering the coordinator's rollback to the `Offered` state.
    app.reconnect(coordinator.info).await.unwrap();

    // Process the app's first `Accept` message. This should fail because of an invalid commitment
    // transaction number.
    coordinator
        .process_incoming_messages()
        .expect_err("should have invalid commitment transaction number");

    // Create second `Accept` message from pending `ReAccept` action.
    sub_channel_manager_periodic_check(
        app.sub_channel_manager.clone(),
        &app.dlc_message_handler,
        &app.peer_manager,
    )
    .await
    .unwrap();

    // Process the app's second `Accept` and send `Confirm`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Confirmed,
    )
    .await
    .unwrap();

    // Process the coordinator's `Confirm` and send `Finalize`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Finalized,
    )
    .await
    .unwrap();

    // Process the app's `Finalize` and send `Revoke`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();

    // Process the coordinator's `Revoke`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_lose_connection_before_processing_subchannel_close_accept() {
    init_tracing();
    // Arrange

    let app_dlc_collateral = 25_000;
    let coordinator_dlc_collateral = 50_000;

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

    coordinator
        .open_private_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await
        .unwrap();

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
        .propose_sub_channel_collaborative_settlement(
            channel_details.channel_id,
            app_dlc_collateral / 2,
        )
        .await
        .unwrap();

    // Process `CloseOffer`.
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::CloseOffered,
    )
    .await
    .unwrap();

    app.accept_sub_channel_collaborative_settlement(&sub_channel.channel_id)
        .unwrap();

    // Give time to deliver the `CloseAccept` message to the coordinator.
    wait_until(Duration::from_secs(5), || async {
        Ok(coordinator
            .dlc_message_handler
            .has_pending_messages_to_process()
            .then_some(()))
    })
    .await
    .unwrap();

    // Lose the connection, triggering re-establishing the channel.
    app.reconnect(coordinator.info).await.unwrap();

    // Process the app's first `CloseAccept` message. This should fail because of an invalid
    // commitment transaction number.
    coordinator
        .process_incoming_messages()
        .expect_err("should have invalid commitment transaction number");

    // Create second `CloseAccept` message from pending `ReCloseAccept` action.
    sub_channel_manager_periodic_check(
        app.sub_channel_manager.clone(),
        &app.dlc_message_handler,
        &app.peer_manager,
    )
    .await
    .unwrap();

    // Process second `CloseAccept` and send `CloseConfirm`.
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::CloseConfirmed,
    )
    .await
    .unwrap();

    // Process the coordinator's `CloseConfirm` and send `CloseFinalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await
    .unwrap();

    // Process the coordinator's `CloseFinalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await
    .unwrap();
}
