use crate::node::Node;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::anyhow;
use bitcoin::Amount;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn given_lightning_channel_then_can_add_dlc_channel() {
    init_tracing();

    // Arrange

    let app = Node::start_test_app("app").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();

    app.keep_connected(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(200_000)).await.unwrap();

    coordinator.open_channel(&app, 50000, 50000).await.unwrap();
    let channel_details = app.channel_manager.list_usable_channels();
    let channel_details = channel_details
        .iter()
        .find(|c| c.counterparty.node_id == coordinator.info.pubkey)
        .unwrap();

    // Act

    let oracle_pk = app.oracle_pk();
    let contract_input = dummy_contract_input(5_000, 2_500, oracle_pk);

    app.propose_dlc_channel(channel_details, &contract_input)
        .await
        .unwrap();

    // TODO: Spawn a task that does this work periodically
    tokio::time::sleep(Duration::from_secs(2)).await;
    coordinator.process_incoming_messages().unwrap();

    let sub_channel = wait_until(Duration::from_secs(30), || async {
        let sub_channels = coordinator
            .dlc_manager
            .get_store()
            .get_sub_channels() // `get_offered_sub_channels` appears to have a bug
            .map_err(|e| anyhow!(e.to_string()))?;

        let sub_channel = sub_channels.iter().find(|sub_channel| {
            sub_channel.counter_party == app.info.pubkey
                && matches!(&sub_channel.state, SubChannelState::Offered(_))
        });

        Ok(sub_channel.cloned())
    })
    .await
    .unwrap();

    coordinator
        .accept_dlc_channel(&sub_channel.channel_id)
        .await
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
        .unwrap();

    matches!(sub_channel_coordinator.state, SubChannelState::Signed(_));

    let sub_channel_app = app
        .dlc_manager
        .get_store()
        .get_sub_channels()
        .unwrap()
        .into_iter()
        .find(|sc| sc.channel_id == sub_channel.channel_id)
        .unwrap();

    matches!(sub_channel_app.state, SubChannelState::Signed(_));
}
