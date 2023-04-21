use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::ChannelId;
use dlc_manager::Storage;
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
    pub channel_id: ChannelId,
}

pub async fn create_dlc_channel(
    app_dlc_collateral: u64,
    coordinator_dlc_collateral: u64,
) -> Result<DlcChannelCreated> {
    // Arrange

    let app_ln_balance = app_dlc_collateral * 2;
    let coordinator_ln_balance = coordinator_dlc_collateral * 2;

    let fund_amount = (app_ln_balance + coordinator_ln_balance) * 2;

    let app = Node::start_test_app("app").await?;
    let coordinator = Node::start_test_coordinator("coordinator").await?;

    app.connect(coordinator.info).await?;

    coordinator.fund(Amount::from_sat(fund_amount)).await?;

    coordinator
        .open_channel(&app, coordinator_ln_balance, app_ln_balance)
        .await?;
    let channel_details = app.channel_manager.list_usable_channels();
    let channel_details = channel_details
        .iter()
        .find(|c| c.counterparty.node_id == coordinator.info.pubkey)
        .context("No usable channels for app")?;
    let channel_id = channel_details.channel_id;

    let app_balance_channel_creation = app.get_ldk_balance().available;
    let coordinator_balance_channel_creation = coordinator.get_ldk_balance().available;

    // Act

    let oracle_pk = app.oracle_pk();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    app.propose_dlc_channel(channel_details, &contract_input)
        .await?;

    // Processs the app's offer to close the channel
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages()?;

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
    .await?;

    coordinator.accept_dlc_channel_offer(&sub_channel.channel_id)?;

    // Process the coordinator's accept message _and_ send the confirm message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages()?;

    // Process the confirm message _and_ send the finalize message
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages()?;

    // Process the finalize message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages()?;

    // Assert

    let sub_channel_coordinator = coordinator
        .dlc_manager
        .get_store()
        .get_sub_channels()?
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for coordinator")?;

    matches!(sub_channel_coordinator.state, SubChannelState::Signed(_));

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()?
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for app")?;

    matches!(sub_channel_app.state, SubChannelState::Signed(_));

    Ok(DlcChannelCreated {
        coordinator,
        coordinator_balance_channel_creation,
        app,
        app_balance_channel_creation,
        channel_id,
    })
}
