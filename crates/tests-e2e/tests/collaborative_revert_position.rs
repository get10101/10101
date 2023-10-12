use coordinator::admin::Balance;
use native::api;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::setup;
use tests_e2e::wait_until;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_revert_channel() {
    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let bitcoin = &test.bitcoind;
    let app = &test.app;

    let app_pubkey = api::get_node_id().unwrap().0;

    let channels = coordinator.get_channels().await.expect("To get channels");

    let channel = channels
        .iter()
        .find(|chan| chan.counterparty == app_pubkey)
        .unwrap();

    let wallet_info = app.rx.wallet_info().expect("To be able to get wallet info");
    assert_eq!(wallet_info.balances.on_chain, 0);

    coordinator
        .sync_wallet()
        .await
        .expect("to be able to sync the wallet");
    let original_balance = coordinator
        .get_balance()
        .await
        .expect("to be able to get balance");
    coordinator
        .collaborative_revert_channel(&channel.channel_id)
        .await
        .expect("To be able to invoke revert");

    // TODO: check for app balance. For that we need to be able to refresh the app on-chain balance
    wait_until!(
        check_for_coordinator_on_chain_balance(coordinator, bitcoin, original_balance.clone())
            .await
    );
}

async fn check_for_coordinator_on_chain_balance(
    coordinator: &Coordinator,
    bitcoin: &Bitcoind,
    original_balance: Balance,
) -> bool {
    bitcoin.mine(1).await.expect("To be able to mine blocks");
    // Let the coordinator catch-up with the blocks
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    coordinator
        .sync_wallet()
        .await
        .expect("to be able to sync the wallet");
    let current_balance = coordinator
        .get_balance()
        .await
        .expect("to be able to get balance");

    current_balance.onchain > original_balance.onchain
}
