use native::api;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::coordinator::SubChannelState;
use tests_e2e::setup;
use tests_e2e::wait_until;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_force_close_position() {
    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let bitcoin = &test.bitcoind;

    let app_pubkey = api::get_node_id().unwrap().0;

    let dlc_channels = coordinator.get_dlc_channels().await.unwrap();

    let dlc_channel_id = dlc_channels
        .iter()
        .find(|chan| chan.counter_party == app_pubkey)
        .unwrap();

    coordinator
        .force_close_channel(&dlc_channel_id.channel_id)
        .await
        .unwrap();

    wait_until!(check_for_channel_closed(coordinator, bitcoin, &app_pubkey).await);

    // TODO: Assert that the position is closed in the app
}

async fn check_for_channel_closed(
    coordinator: &Coordinator,
    bitcoin: &Bitcoind,
    app_pubkey: &str,
) -> bool {
    bitcoin.mine(100).await.expect("To be able to mine blocks");
    // Let the coordinator catch-up with the blocks
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    coordinator
        .get_dlc_channels()
        .await
        .expect("to be able to retrieve dlc channels from coordinator")
        .iter()
        .any(|chan| {
            chan.counter_party == app_pubkey && chan.state == SubChannelState::OnChainClosed
        })
}
