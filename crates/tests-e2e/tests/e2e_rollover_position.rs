#![allow(clippy::unwrap_used)]

use bitcoin::Network;
use native::api;
use native::api::ChannelState;
use native::api::SignedChannelState;
use native::trade::position;
use position::PositionState;
use rust_decimal_macros::dec;
use tests_e2e::app::force_close_dlc_channel;
use tests_e2e::app::get_dlc_channels;
use tests_e2e::app::AppHandle;
use tests_e2e::coordinator;
use tests_e2e::coordinator::FundingRate;
use tests_e2e::coordinator::FundingRates;
use tests_e2e::setup;
use tests_e2e::wait_until;
use time::ext::NumericalDuration;
use time::OffsetDateTime;
use xxi_node::commons;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_rollover_position() {
    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let dlc_channels = coordinator.get_dlc_channels().await.unwrap();
    let app_pubkey = api::get_node_id().0;

    let position_coordinator_before =
        coordinator.get_positions(&app_pubkey).await.unwrap()[0].clone();

    tracing::info!("{:?}", dlc_channels);

    let dlc_channel = dlc_channels
        .into_iter()
        .find(|chan| chan.counter_party == app_pubkey)
        .unwrap();

    let new_expiry = commons::calculate_next_expiry(OffsetDateTime::now_utc(), Network::Regtest);

    generate_outstanding_funding_fee_event(&test, &app_pubkey, position_coordinator_before.id)
        .await;

    coordinator
        .rollover(&dlc_channel.dlc_channel_id.unwrap())
        .await
        .unwrap();

    wait_until!(check_rollover_position(&test.app, new_expiry));
    wait_until!(test
        .app
        .rx
        .position()
        .map(|p| PositionState::Open == p.position_state)
        .unwrap_or(false));

    wait_until_funding_fee_event_is_paid(&test, &app_pubkey, position_coordinator_before.id).await;

    let position_coordinator_after =
        coordinator.get_positions(&app_pubkey).await.unwrap()[0].clone();

    verify_coordinator_position_after_rollover(
        &position_coordinator_before,
        &position_coordinator_after,
        new_expiry,
    );

    // Once the rollover is complete, we also want to verify that the channel can still be
    // force-closed. This should be tested in `rust-dlc`, but we recently encountered a bug in our
    // branch: https://github.com/get10101/10101/pull/2079.

    force_close_dlc_channel(&test.bitcoind).await;

    let channels = get_dlc_channels();
    let channel = channels.first().unwrap();

    wait_until!(matches!(
        channel.channel_state,
        ChannelState::Signed {
            state: SignedChannelState::Closing { .. },
            ..
        }
    ));
}

fn check_rollover_position(app: &AppHandle, new_expiry: OffsetDateTime) -> bool {
    let position = app.rx.position().unwrap();
    tracing::debug!(
        "expect {:?} to be {:?}",
        position.position_state,
        PositionState::Rollover
    );
    tracing::debug!(
        "expect {} to be {}",
        position.expiry.unix_timestamp(),
        new_expiry.unix_timestamp()
    );

    PositionState::Rollover == position.position_state
        && new_expiry.unix_timestamp() == position.expiry.unix_timestamp()
}

/// Verify the coordinator's position after executing a rollover, given that a funding fee was paid
/// from the trader to the coordinator.
fn verify_coordinator_position_after_rollover(
    before: &coordinator::Position,
    after: &coordinator::Position,
    new_expiry: OffsetDateTime,
) {
    assert_eq!(after.position_state, coordinator::PositionState::Open);

    assert_eq!(before.quantity, after.quantity);
    assert_eq!(before.trader_direction, after.trader_direction);
    assert_eq!(before.average_entry_price, after.average_entry_price);
    assert_eq!(before.coordinator_leverage, after.coordinator_leverage);
    assert_eq!(
        before.coordinator_liquidation_price,
        after.coordinator_liquidation_price
    );
    assert_eq!(before.coordinator_margin, after.coordinator_margin);
    assert_eq!(before.contract_symbol, after.contract_symbol);
    assert_eq!(before.order_matching_fees, after.order_matching_fees);

    assert_eq!(after.expiry_timestamp, new_expiry);

    insta::assert_json_snapshot!(after, {
        ".id" => "[u64]".to_string(),
        ".creation_timestamp" => "[timestamp]".to_string(),
        ".update_timestamp" => "[timestamp]".to_string(),
        ".expiry_timestamp" => "[timestamp]".to_string(),
        ".trader_pubkey" => "[public-key]".to_string(),
        ".temporary_contract_id" => "[public-key]".to_string(),
    });
}

async fn generate_outstanding_funding_fee_event(
    test: &setup::TestSetup,
    node_id_app: &str,
    position_id: u64,
) {
    let end_date = OffsetDateTime::now_utc() - 1.minutes();
    let start_date = end_date - 8.hours();

    // Let coordinator know about past funding rate.
    test.coordinator
        .post_funding_rates(FundingRates(vec![FundingRate {
            // The trader will owe the coordinator.
            rate: dec!(0.001),
            start_date,
            end_date,
        }]))
        .await
        .unwrap();

    // Make the coordinator think that the trader's position was created before the funding period
    // ended.
    test.coordinator
        .modify_position_creation_timestamp(end_date - 1.hours(), node_id_app)
        .await
        .unwrap();

    wait_until_funding_fee_event_is_created(test, node_id_app, position_id).await;
}

async fn wait_until_funding_fee_event_is_created(
    test: &setup::TestSetup,
    node_id_app: &str,
    position_id: u64,
) {
    wait_until!({
        test.coordinator
            .get_funding_fee_events(node_id_app, position_id)
            .await
            .unwrap()
            .first()
            .is_some()
    });
}

async fn wait_until_funding_fee_event_is_paid(
    test: &setup::TestSetup,
    node_id_app: &str,
    position_id: u64,
) {
    wait_until!({
        let funding_fee_events = test
            .coordinator
            .get_funding_fee_events(node_id_app, position_id)
            .await
            .unwrap();

        funding_fee_events
            .iter()
            .all(|event| event.paid_date.is_some())
    });
}
