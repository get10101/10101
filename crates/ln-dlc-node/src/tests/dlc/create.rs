use crate::node::InMemoryStore;
use crate::node::Node;
use crate::storage::TenTenOneInMemoryStorage;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_for_n_usable_channels;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn given_lightning_channel_then_can_add_dlc_channel() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

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

    // Act and assert

    create_dlc_channel(
        &app,
        &coordinator,
        app_dlc_collateral,
        coordinator_dlc_collateral,
    )
    .await
    .unwrap();
}

pub async fn create_dlc_channel(
    offer_node: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    accept_node: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    app_dlc_collateral: u64,
    coordinator_dlc_collateral: u64,
) -> Result<()> {
    // Act

    let oracle_pk = *offer_node.oracle_pk().first().unwrap();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    wait_for_n_usable_channels(1, offer_node).await?;
    let channel_details = offer_node
        .channel_manager
        .list_usable_channels()
        .iter()
        .find(|c| c.counterparty.node_id == accept_node.info.pubkey)
        .context("Could not find usable channel with peer")?
        .clone();

    offer_node
        .propose_sub_channel(channel_details.clone(), contract_input)
        .await?;

    // Process the app's `Offer`
    let sub_channel = wait_until_dlc_channel_state(
        Duration::from_secs(30),
        accept_node,
        offer_node.info.pubkey,
        SubChannelStateName::Offered,
    )
    .await?;

    accept_node.accept_sub_channel_offer(&sub_channel.channel_id)?;

    // Process the coordinator's `Accept` and send `Confirm`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        offer_node,
        accept_node.info.pubkey,
        SubChannelStateName::Confirmed,
    )
    .await?;

    // Process the app's `Confirm` and send `Finalize`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        accept_node,
        offer_node.info.pubkey,
        SubChannelStateName::Finalized,
    )
    .await?;

    // Assert

    // Process the coordinator's `Finalize` and send `Revoke`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        offer_node,
        accept_node.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await?;

    // Process the app's `Revoke`
    wait_until_dlc_channel_state(
        Duration::from_secs(30),
        accept_node,
        offer_node.info.pubkey,
        SubChannelStateName::Signed,
    )
    .await?;

    Ok(())
}
