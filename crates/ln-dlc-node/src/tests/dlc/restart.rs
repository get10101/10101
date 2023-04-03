use crate::ln::app_config;
use crate::ln::coordinator_config;
use crate::node::Node;
use crate::seed::Bip39Seed;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::anyhow;
use anyhow::Context;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn create_dlc() {
    init_tracing();

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    let name = "app";

    let tmp_dir = "/tmp/ln-dlc-restart-test/";
    let tmp_dir = PathBuf::from(tmp_dir);
    let data_dir = tmp_dir.join(name);

    std::fs::create_dir_all(&data_dir).unwrap();

    let seed = Bip39Seed::initialize(&data_dir.join("seed")).unwrap();

    let app = Node::start_test(name, app_config(), Some(&data_dir), Some(seed))
        .await
        .unwrap();

    let name = "coordinator";
    let data_dir = tmp_dir.join(name);

    std::fs::create_dir_all(&data_dir).unwrap();

    let seed = Bip39Seed::initialize(&data_dir.join("seed")).unwrap();

    let coordinator = Node::start_test(name, coordinator_config(), Some(&data_dir), Some(seed))
        .await
        .unwrap();

    let DlcChannelCreated {
        app, coordinator, ..
    } = create_dlc_channel(
        app_dlc_collateral,
        coordinator_dlc_collateral,
        Some(app),
        Some(coordinator),
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn close_dlc_probably_fails() {
    init_tracing();

    let coordinator_dlc_collateral = 25_000;

    let name = "app";

    let tmp_dir = "/tmp/ln-dlc-restart-test/";
    let tmp_dir = PathBuf::from(tmp_dir);
    let data_dir = tmp_dir.join(name);

    let seed = Bip39Seed::initialize(&data_dir.join("seed")).unwrap();

    let app = Node::start_test(name, app_config(), Some(&data_dir), Some(seed))
        .await
        .unwrap();

    let name = "coordinator";
    let data_dir = tmp_dir.join(name);

    let seed = Bip39Seed::initialize(&data_dir.join("seed")).unwrap();

    let coordinator = Node::start_test(name, coordinator_config(), Some(&data_dir), Some(seed))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(10)).await;

    // Act

    // The underlying API expects the settlement amount of the party who originally _accepted_ the
    // channel. Since we know in this case that the coordinator accepted the DLC channel, here we
    // specify the coordinator's settlement amount.
    app.connect_to_peer(coordinator.info).await.unwrap();

    let coordinator_settlement_amount = coordinator_dlc_collateral / 2;

    tokio::time::sleep(Duration::from_secs(10)).await;

    let channel_details = app.channel_manager.list_usable_channels();
    let channel_details = channel_details
        .iter()
        .find(|c| c.counterparty.node_id == coordinator.info.pubkey)
        .context("No usable channels for app")
        .unwrap();
    let channel_id = channel_details.channel_id;

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
            .map_err(|e| anyhow!(e.to_string()))
            .unwrap();

        let sub_channel = sub_channels.iter().find(|sub_channel| {
            sub_channel.counter_party == app.info.pubkey
                && matches!(&sub_channel.state, SubChannelState::CloseOffered(_))
        });

        Ok(sub_channel.cloned())
    })
    .await
    .unwrap();

    coordinator
        .accept_dlc_channel_collaborative_settlement(&sub_channel.channel_id)
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
        .map_err(|e| anyhow!(e.to_string()))
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for coordinator")
        .unwrap();

    matches!(
        sub_channel_coordinator.state,
        SubChannelState::OffChainClosed
    );

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .map_err(|e| anyhow!(e.to_string()))
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .context("No DLC channel for app")
        .unwrap();

    matches!(sub_channel_app.state, SubChannelState::OffChainClosed);
}
