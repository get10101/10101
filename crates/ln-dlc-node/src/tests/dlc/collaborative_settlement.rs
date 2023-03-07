use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::anyhow;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn dlc_collaborative_settlement() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    let DlcChannelCreated {
        coordinator,
        coordinator_balance_channel_creation,
        app,
        app_balance_channel_creation,
        channel_id,
    } = create_dlc_channel(app_dlc_collateral, coordinator_dlc_collateral)
        .await
        .unwrap();

    // TODO: Figure out why the balances look like they do after the DLC channel is open

    tracing::info!(balance = ?app.get_ldk_balance(), "App balance after opening DLC channel");
    tracing::info!(balance = ?coordinator.get_ldk_balance(), "Coordinator balance after opening DLC channel");

    // Act

    // The underlying API expects the settlement amount of the party who originally _accepted_ the
    // channel. Since we know in this case that the coordinator accepted the DLC channel, here we
    // specify the coordinator's settlement amount.
    let coordinator_settlement_amount = 10_000;
    let coordinator_loss_amount = coordinator_dlc_collateral - coordinator_settlement_amount;

    app.propose_dlc_channel_collaborative_settlement(&channel_id, coordinator_settlement_amount)
        .unwrap();

    // Processs the app's offer to close the channel
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    let sub_channel = wait_until(Duration::from_secs(30), || async {
        let sub_channels = coordinator
            .dlc_manager
            .get_store()
            .get_sub_channels()
            .map_err(|e| anyhow!(e.to_string()))?;

        let sub_channel = sub_channels.iter().find(|sub_channel| {
            sub_channel.counter_party == app.info.pubkey
                && matches!(&sub_channel.state, SubChannelState::CloseOffered(_))
        });

        Ok(sub_channel.cloned())
    })
    .await
    .unwrap();

    coordinator
        .initiate_accept_dlc_channel_close_offer(&sub_channel.channel_id)
        .unwrap();

    // Process the coordinator's accept message _and_ send the confirm
    // message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages().unwrap();

    // Process the confirm message
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    // Process the close-finalize message
    tokio::time::sleep(Duration::from_secs(2)).await;
    app.process_incoming_messages().unwrap();

    // Assert

    let sub_channel_coordinator = coordinator
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .unwrap();

    matches!(
        sub_channel_coordinator.state,
        SubChannelState::OffChainClosed
    );

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .map_err(|e| anyhow!("{e}"))
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .unwrap();

    matches!(sub_channel_app.state, SubChannelState::OffChainClosed);

    let app_balance_after = app.get_ldk_balance().available;

    assert_eq!(
        app_balance_channel_creation + coordinator_loss_amount,
        app_balance_after
    );

    let coordinator_balance_after = coordinator.get_ldk_balance().available;

    assert_eq!(
        coordinator_balance_channel_creation,
        coordinator_balance_after + coordinator_loss_amount
    );
}
