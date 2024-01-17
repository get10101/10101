#![allow(clippy::unwrap_used)]

use native::api;
use native::api::ContractSymbol;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use tests_e2e::setup;
use tests_e2e::setup::dummy_order;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

// Comments are based on a fixed price of 40_000.
// TODO: Add assertions when the maker price can be fixed.

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_open_close_open_close_position() {
    let test = setup::TestSetup::new_with_open_position().await;

    // - App margin is 1_250_000 sats.
    // - Opening fee of 7_500 paid to coordinator collateral reserve from app on-chain balance.
    // - App off-chain balance is 0 (first trade uses full DLC channel collateral for now).

    let app_off_chain_balance = test.app.rx.wallet_info().unwrap().balances.off_chain;
    tracing::info!(%app_off_chain_balance, "Opened first position");

    let closing_order = {
        let mut order = dummy_order();
        order.direction = api::Direction::Short;
        order
    };

    tracing::info!("Closing first position");

    spawn_blocking(move || api::submit_order(closing_order).unwrap())
        .await
        .unwrap();

    wait_until!(test.app.rx.position_close().is_some());

    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // - App off-chain balance is 1_242_500 sats (margin minus 7_500 fee).

    let app_off_chain_balance = test.app.rx.wallet_info().unwrap().balances.off_chain;
    tracing::info!(%app_off_chain_balance, "Closed first position");

    tracing::info!("Opening second position");

    let order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 500.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    wait_until!(test.app.rx.position().is_some());
    wait_until!(test.app.rx.position().unwrap().position_state == PositionState::Open);

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // - App margin is 625_000 sats.
    // - Opening fee of 3_750 paid to coordinator collateral reserve from app off-chain balance.
    // - App off-chain balance is 613_750.

    let app_off_chain_balance = test.app.rx.wallet_info().unwrap().balances.off_chain;
    tracing::info!(%app_off_chain_balance, "Opened second position");

    tracing::info!("Closing second position");

    let closing_order = NewOrder {
        direction: api::Direction::Short,
        ..order
    };

    spawn_blocking(move || api::submit_order(closing_order).unwrap())
        .await
        .unwrap();

    wait_until!(test.app.rx.position_close().is_some());

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // - App off-chain balance is 1_235_000 sats (reserve + margin - 3_750 fee).

    let app_off_chain_balance = test.app.rx.wallet_info().unwrap().balances.off_chain;
    tracing::info!(%app_off_chain_balance, "Closed second position");

    // TODO: Assert that the position is closed in the coordinator
}
