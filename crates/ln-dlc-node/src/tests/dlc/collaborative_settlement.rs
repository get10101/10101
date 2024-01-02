use crate::node::InMemoryStore;
use crate::node::Node;
use crate::storage::TenTenOneInMemoryStorage;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::init_tracing;
use crate::tests::wait_until_sub_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn dlc_collaborative_settlement_test() {
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

    let app_balance_channel_creation = app.get_ldk_balance().available();
    let coordinator_balance_channel_creation = coordinator.get_ldk_balance().available();

    create_dlc_channel(
        &app,
        &coordinator,
        app_dlc_collateral,
        coordinator_dlc_collateral,
    )
    .await
    .unwrap();

    // The underlying API expects the settlement amount of the party who originally _accepted_ the
    // channel. Since we know in this case that the coordinator accepted the DLC channel, here we
    // specify the coordinator's settlement amount.
    let coordinator_settlement_amount = coordinator_dlc_collateral / 2;

    // Act

    dlc_collaborative_settlement(&app, &coordinator, coordinator_settlement_amount)
        .await
        .unwrap();

    // Assert

    let coordinator_loss_amount = coordinator_dlc_collateral - coordinator_settlement_amount;

    let app_balance_after = app.get_ldk_balance().available();
    let coordinator_balance_after = coordinator.get_ldk_balance().available();

    assert_eq!(
        app_balance_channel_creation + coordinator_loss_amount,
        app_balance_after
    );

    assert_eq!(
        coordinator_balance_channel_creation,
        coordinator_balance_after + coordinator_loss_amount
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn open_dlc_channel_after_closing_dlc_channel() {
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

    create_dlc_channel(
        &app,
        &coordinator,
        app_dlc_collateral,
        coordinator_dlc_collateral,
    )
    .await
    .unwrap();

    let coordinator_settlement_amount = coordinator_dlc_collateral / 2;
    dlc_collaborative_settlement(&app, &coordinator, coordinator_settlement_amount)
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

async fn dlc_collaborative_settlement(
    app: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    coordinator: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    coordinator_settlement_amount: u64,
) -> Result<()> {
    let channel_details = app
        .channel_manager
        .list_usable_channels()
        .iter()
        .find(|c| c.counterparty.node_id == coordinator.info.pubkey)
        .context("Could not find usable channel with peer")?
        .clone();

    app.propose_sub_channel_collaborative_settlement(
        channel_details.channel_id,
        coordinator_settlement_amount,
    )
    .await?;

    // Process the app's `CloseOffer`
    let sub_channel = wait_until_sub_channel_state(
        Duration::from_secs(30),
        coordinator,
        app.info.pubkey,
        SubChannelStateName::CloseOffered,
    )
    .await?;

    coordinator.accept_sub_channel_collaborative_settlement(&sub_channel.channel_id)?;

    // Process the coordinator's `CloseAccept` and send `CloseConfirm`
    wait_until_sub_channel_state(
        Duration::from_secs(30),
        app,
        coordinator.info.pubkey,
        SubChannelStateName::CloseConfirmed,
    )
    .await?;

    // Assert

    // Process the app's `CloseConfirm` and send `CloseFinalize`
    wait_until_sub_channel_state(
        Duration::from_secs(30),
        coordinator,
        app.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await?;

    // Process the coordinator's `CloseFinalize`
    wait_until_sub_channel_state(
        Duration::from_secs(30),
        app,
        coordinator.info.pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await?;

    Ok(())
}
