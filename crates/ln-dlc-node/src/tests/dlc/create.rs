use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use lightning::ln::channelmanager::ChannelDetails;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn given_lightning_channel_then_can_add_dlc_channel() {
    init_tracing();

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    create_dlc_channel(app_dlc_collateral, coordinator_dlc_collateral)
        .await
        .unwrap();
}

pub struct DlcChannelCreated {
    pub coordinator: Node<PaymentMap>,
    /// Available balance for the coordinator after the LN channel was created. In sats.
    pub coordinator_balance_channel_creation: u64,
    pub app: Node<PaymentMap>,
    /// Available balance for the app after the LN channel was created. In sats.
    pub app_balance_channel_creation: u64,
    pub channel_details: ChannelDetails,
}

pub async fn create_dlc_channel(
    app_dlc_collateral: u64,
    coordinator_dlc_collateral: u64,
) -> Result<DlcChannelCreated> {
    // Arrange

    let app_ln_balance = app_dlc_collateral * 2;
    let coordinator_ln_balance = coordinator_dlc_collateral * 2;

    let fund_amount = (app_ln_balance + coordinator_ln_balance) * 2;

    let app = Node::start_test_app("app")?;
    let coordinator = Node::start_test_coordinator("coordinator")?;

    app.connect(coordinator.info).await?;

    coordinator.fund(Amount::from_sat(fund_amount)).await?;

    coordinator
        .open_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await?;
    let channel_details = app.channel_manager.list_usable_channels();
    let channel_details = channel_details
        .into_iter()
        .find(|c| c.counterparty.node_id == coordinator.info.pubkey)
        .context("No usable channels for app")?;

    let app_balance_channel_creation = app.get_ldk_balance().available;
    let coordinator_balance_channel_creation = coordinator.get_ldk_balance().available;

    // Act

    let oracle_pk = app.oracle_pk();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    app.propose_dlc_channel(channel_details.clone(), contract_input)
        .await?;

    // Process the app's `Offer`
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        &coordinator,
        app.info.pubkey,
        SubChannelStateName::Offered,
    )
    .await
    .unwrap();

    coordinator.accept_dlc_channel_offer(&sub_channel.channel_id)?;

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

    Ok(DlcChannelCreated {
        coordinator,
        coordinator_balance_channel_creation,
        app,
        app_balance_channel_creation,
        channel_details,
    })
}
