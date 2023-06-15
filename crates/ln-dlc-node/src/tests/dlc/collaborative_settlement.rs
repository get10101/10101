use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use anyhow::Result;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn dlc_collaborative_settlement_test() {
    init_tracing();

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    dlc_collaborative_settlement(app_dlc_collateral, coordinator_dlc_collateral)
        .await
        .unwrap();
}

/// Start an app and a coordinator; create an LN channel between them with double the specified
/// amounts; add a DLC channel with the specified amounts; and close the DLC channel giving the
/// coordinator 50% losses.
async fn dlc_collaborative_settlement(
    app_dlc_collateral: u64,
    coordinator_dlc_collateral: u64,
) -> Result<(Node<PaymentMap>, Node<PaymentMap>)> {
    // Arrange

    let DlcChannelCreated {
        coordinator,
        coordinator_balance_channel_creation,
        app,
        app_balance_channel_creation,
        channel_details,
    } = create_dlc_channel(app_dlc_collateral, coordinator_dlc_collateral).await?;

    // Act

    // The underlying API expects the settlement amount of the party who originally _accepted_ the
    // channel. Since we know in this case that the coordinator accepted the DLC channel, here we
    // specify the coordinator's settlement amount.
    let coordinator_settlement_amount = coordinator_dlc_collateral / 2;
    let coordinator_loss_amount = coordinator_dlc_collateral - coordinator_settlement_amount;

    app.propose_dlc_channel_collaborative_settlement(
        channel_details.channel_id,
        coordinator_settlement_amount,
    )
    .await?;

    // Process the app's `CloseOffer`
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::CloseOffered,
    )
    .await?;

    coordinator.accept_dlc_channel_collaborative_settlement(&sub_channel.channel_id)?;

    // Process the coordinator's `CloseAccept` and send `CloseConfirm`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::CloseConfirmed,
    )
    .await?;

    // Assert

    // Process the app's `CloseConfirm` and send `CloseFinalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await?;

    // Process the coordinator's `CloseFinalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await?;

    let app_balance_after = app.get_ldk_balance().available;
    let coordinator_balance_after = coordinator.get_ldk_balance().available;

    assert_eq!(
        app_balance_channel_creation + coordinator_loss_amount,
        app_balance_after
    );

    assert_eq!(
        coordinator_balance_channel_creation,
        coordinator_balance_after + coordinator_loss_amount
    );

    Ok((app, coordinator))
}

#[tokio::test]
#[ignore]
async fn open_dlc_channel_after_closing_dlc_channel() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    let (app, coordinator) =
        dlc_collaborative_settlement(app_dlc_collateral, coordinator_dlc_collateral)
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

    let app_dlc_collateral = 20_000;
    let coordinator_dlc_collateral = 10_000;

    let oracle_pk = app.oracle_pk();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    app.propose_dlc_channel(channel_details, contract_input)
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

    coordinator
        .accept_dlc_channel_offer(&sub_channel.channel_id)
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

    // Assert

    // Process the app's `Confirm` and send `Finalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();

    // Process the coordinator's `Finalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &app,
        coordinator.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await
    .unwrap();
}
