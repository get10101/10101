#![allow(clippy::unwrap_used)]

use bitcoin::Txid;
use coordinator::admin::Balance;
use native::api;
use rust_decimal_macros::dec;
use std::str::FromStr;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::setup;
use tests_e2e::wait_until;
use tokio::task::block_in_place;

// TODO: We should check that the on-chain balances have increased by the _expected amount_ for
// these tests.

// Use `flavor = "multi_thread"` to be able to call `block_in_place`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_revert_channel_without_coordinator_channel_details() {
    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let bitcoin = &test.bitcoind;
    let app = &test.app;

    let app_pubkey = api::get_node_id().0;

    let channels = coordinator.get_channels().await.unwrap();
    let channel = channels
        .iter()
        .find(|chan| chan.counterparty == app_pubkey)
        .unwrap();

    let wallet_info = app.rx.wallet_info().unwrap();
    assert_eq!(wallet_info.balances.on_chain, 0);

    let original_funding_txo = {
        let original_funding_txo = channel.original_funding_txo.clone();
        let funding_txo = channel.funding_txo.clone();

        original_funding_txo.or(funding_txo).unwrap()
    };

    let split: Vec<_> = original_funding_txo.split(':').collect();
    let txid = Txid::from_str(split[0]).unwrap();
    let vout = u32::from_str(split[1]).unwrap();

    coordinator.sync_wallet().await.unwrap();
    let coordinator_balance_before = coordinator.get_balance().await.unwrap();

    let app_balance_before = app.rx.wallet_info().unwrap().balances.on_chain;

    // The price is only informational for the app. The app will display things based on the price,
    // but we are not asserting on that in this test. Hence, we can use an arbitrary value.
    let price = dec!(30_000);

    // We settle at an arbitrary price. We at least choose a value that we know will be valid.
    let coordinator_amount_sat = channel.outbound_capacity_msat / 1_000;

    coordinator
        .expert_collaborative_revert_channel(
            &channel.channel_id,
            coordinator_amount_sat,
            price,
            txid,
            vout,
        )
        .await
        .unwrap();

    wait_until!(
        check_for_coordinator_on_chain_balance(
            coordinator,
            bitcoin,
            coordinator_balance_before.clone()
        )
        .await
    );

    // We must `block_in_place` because calling `refresh_wallet_info` starts a new runtime and that
    // cannot happen within another runtime.
    block_in_place(move || api::refresh_wallet_info().unwrap());

    wait_until!(app.rx.wallet_info().unwrap().balances.on_chain > app_balance_before);
}

async fn check_for_coordinator_on_chain_balance(
    coordinator: &Coordinator,
    bitcoin: &Bitcoind,
    original_balance: Balance,
) -> bool {
    bitcoin.mine(1).await.unwrap();

    // Let the coordinator catch up with the new block.
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    coordinator.sync_wallet().await.unwrap();
    let current_balance = coordinator.get_balance().await.unwrap();

    current_balance.onchain > original_balance.onchain
}
