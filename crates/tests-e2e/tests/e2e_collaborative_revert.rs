#![allow(clippy::unwrap_used)]

use native::api::PaymentFlow;
use native::api::WalletHistoryItemType;
use rust_decimal_macros::dec;
use tests_e2e::app::get_dlc_channel_id;
use tests_e2e::app::refresh_wallet_info;
use tests_e2e::app::sync_dlc_channels;
use tests_e2e::coordinator::CollaborativeRevertCoordinatorRequest;
use tests_e2e::setup;
use tests_e2e::wait_until;

// Use `flavor = "multi_thread"` to be able to call `block_in_place`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_revert_channel() {
    // Arrange

    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let bitcoin = &test.bitcoind;
    let app = &test.app;

    let position = app.rx.position().unwrap();
    let app_margin = position.collateral;

    let dlc_channel_id = get_dlc_channel_id().unwrap();

    let app_balance_before = app.rx.wallet_info().unwrap().balances.on_chain;

    // Act

    let collaborative_revert_app_payout = app_margin / 2;

    coordinator
        .collaborative_revert(CollaborativeRevertCoordinatorRequest {
            channel_id: dlc_channel_id,
            counter_payout: collaborative_revert_app_payout,
            price: dec!(40_000),
            fee_rate_sats_vb: 1,
        })
        .await
        .unwrap();

    // Assert

    wait_until!({
        bitcoin.mine(1).await.unwrap();

        sync_dlc_channels();
        refresh_wallet_info();

        let app_balance = app.rx.wallet_info().unwrap().balances.on_chain;

        tracing::debug!(
            before = %app_balance_before,
            now = %app_balance,
            "Checking on-chain balance"
        );

        app_balance > app_balance_before
    });

    let wallet_info = app.rx.wallet_info().unwrap();
    let collab_revert_entry = wallet_info
        .history
        .iter()
        .filter(|entry| {
            matches!(entry.flow, PaymentFlow::Inbound)
                && matches!(entry.wallet_type, WalletHistoryItemType::OnChain { .. })
        })
        .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
        .unwrap();

    let total_tx_fee = match collab_revert_entry.wallet_type {
        WalletHistoryItemType::OnChain {
            fee_sats: Some(fee_sats),
            ..
        } => fee_sats,
        _ => unreachable!(),
    };

    // The transaction fee for the collaborative revert transaction is split evenly among the two
    // parties.
    let tx_fee = total_tx_fee / 2;

    let expected_payout = collaborative_revert_app_payout - tx_fee;

    assert_eq!(collab_revert_entry.amount_sats, expected_payout);

    // TODO: Check coordinator balance too.
}
