use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::Context;
use anyhow::Result;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
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
        channel_id,
    } = create_dlc_channel(app_dlc_collateral, coordinator_dlc_collateral).await?;

    // Act

    // The underlying API expects the settlement amount of the party who originally _accepted_ the
    // channel. Since we know in this case that the coordinator accepted the DLC channel, here we
    // specify the coordinator's settlement amount.
    let coordinator_settlement_amount = coordinator_dlc_collateral / 2;
    let coordinator_loss_amount = coordinator_dlc_collateral - coordinator_settlement_amount;

    app.propose_dlc_channel_collaborative_settlement(&channel_id, coordinator_settlement_amount)?;

    // Processs the app's offer to close the channel
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages()?;

    let sub_channel = wait_until(Duration::from_secs(30), || async {
        let sub_channels = coordinator.dlc_manager.get_store().get_sub_channels()?;

        let sub_channel = sub_channels.iter().find(|sub_channel| {
            sub_channel.counter_party == app.info.pubkey
                && matches!(&sub_channel.state, SubChannelState::CloseOffered(_))
        });

        Ok(sub_channel.cloned())
    })
    .await?;

    coordinator.accept_dlc_channel_collaborative_settlement(&sub_channel.channel_id)?;

    // Process the coordinator's accept message _and_ send the confirm
    // message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages()?;

    // Process the confirm message
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages()?;

    // Process the close-finalize message
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

    matches!(
        sub_channel_coordinator.state,
        SubChannelState::OffChainClosed
    );

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()?
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for app")?;

    let app_balance_after = app.get_ldk_balance().available;
    let coordinator_balance_after = coordinator.get_ldk_balance().available;

    matches!(sub_channel_app.state, SubChannelState::OffChainClosed);

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

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
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
        .unwrap();

    // Act

    let app_dlc_collateral = 20_000;
    let coordinator_dlc_collateral = 10_000;

    let oracle_pk = app.oracle_pk();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    app.propose_dlc_channel(channel_details, &contract_input)
        .await
        .unwrap();

    // Processs the app's offer to close the channel
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

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

    // Process the coordinator's accept message _and_ send the confirm
    // message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages().unwrap();

    // Process the confirm message
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    // Assert

    let sub_channel_coordinator = coordinator
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for coordinator")
        .unwrap();

    matches!(sub_channel_coordinator.state, SubChannelState::Signed(_));

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for app")
        .unwrap();

    matches!(sub_channel_app.state, SubChannelState::Signed(_));
}
