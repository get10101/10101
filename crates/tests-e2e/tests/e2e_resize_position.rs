#![allow(clippy::unwrap_used)]

use native::api::ContractSymbol;
use native::api::Direction;
use native::api::PaymentFlow;
use native::api::WalletHistoryItemType;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use tests_e2e::app::submit_order;
use tests_e2e::setup;
use tests_e2e::setup::TestSetup;
use tests_e2e::wait_until;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_resize_position() {
    let position_direction = Direction::Short;
    let order = NewOrder {
        leverage: 2.0,
        quantity: 250.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: position_direction,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    let test =
        setup::TestSetup::new_with_open_position_custom(order.clone(), 1_000_000, 1_000_000).await;

    let off_chain_balance_after_open = test
        .app
        .rx
        .wallet_info()
        .unwrap()
        .balances
        .off_chain
        .unwrap();

    // We start with 1_000_000 sats in the reserve and 250_005 sats as DLC margin.
    tracing::info!(
        app_off_chain_balance = %off_chain_balance_after_open,
        position = ?test.app.rx.position(),
        "Opened position"
    );

    let increasing_order = NewOrder {
        quantity: 250.0,
        ..order.clone()
    };

    tracing::info!(?increasing_order, "Increasing position");

    let order_id = submit_order(increasing_order);
    wait_until_position_matches(&test, 500, Direction::Short).await;

    // To increase the position, we must increase the margin (and decrease the reserve).
    //
    // 250_005 [extra margin] = 250 [order contracts] / (49_999 [price] * 2 [leverage])
    //
    // 1_500 [fee] = 250 [order contracts] * (1 / 49_999 [price]) * 0.0030 [fee coefficient]
    //
    // 748_495 [new reserve] = 1_000_000 [reserve] - 250_005 [extra margin] - 1_500 [fee]
    let expected_off_chain_balance = 748_495;
    wait_until_balance_equals(&test, expected_off_chain_balance).await;

    // -251_505 [trade amount] = -250_005 [extra margin] - 1_500 [fee]
    check_trade(
        &test,
        &order_id,
        Direction::Short,
        250,
        None,
        1_500,
        -251_505,
    );

    tracing::info!(
        app_off_chain_balance = %expected_off_chain_balance,
        position = ?test.app.rx.position(),
        "Increased position"
    );

    let decreasing_order = NewOrder {
        quantity: 400.0,
        direction: order.direction.opposite(),
        ..order.clone()
    };

    tracing::info!(?decreasing_order, "Decreasing position");

    let order_id = submit_order(decreasing_order);
    wait_until_position_matches(&test, 100, Direction::Short).await;

    // To decrease the position, we must decrease the margin (and increase the reserve).
    //
    // 400_008 [margin reduction] = 400 [order contracts] / (49_999 [opening price] * 2 [leverage])
    //
    // 2_400 [fee] = 400 [order contracts] * (1 / 50_001 [closing price]) * 0.0030 [fee coefficient]
    //
    // -32 [pnl] = (400 [order contracts] / 50_001 [closing price]) - (400 [order contracts] /
    // 49_999 [opening price])
    //
    // 1_146_071 [new reserve] = 748_495 [reserve] + 400_008 [margin reduction] - 2_400 [fee] - 32
    // [pnl]
    let expected_off_chain_balance = 1_146_071;
    wait_until_balance_equals(&test, expected_off_chain_balance).await;

    // 397_576 [trade amount] = 400_008 [margin reduction] - 2_400 [fee] - 32 [pnl]
    check_trade(
        &test,
        &order_id,
        Direction::Long,
        400,
        Some(-32),
        2_400,
        397_576,
    );

    tracing::info!(
        app_off_chain_balance = %expected_off_chain_balance,
        position = ?test.app.rx.position(),
        "Decreased position"
    );

    let direction_changing_order = NewOrder {
        quantity: 300.0,
        direction: order.direction.opposite(),
        ..order
    };

    tracing::info!(?direction_changing_order, "Changing position direction");

    let order_id = submit_order(direction_changing_order);
    wait_until_position_matches(&test, 200, Direction::Long).await;

    // To change direction, we must decrease the margin to 0 and then increase it. The total effect
    // depends on the specific order executed.
    //
    // 100_002 [closed margin] = 100 [close contracts] / (49_999 [opening price] * 2 [leverage])
    //
    // -8 [pnl] = (100 [close contracts] / 50_001 [closing price]) - (100 [close contracts] / 49_999
    // [opening price])
    //
    // 199_996 [new margin] = 200 [remaining contracts] / (50_001 [price] * 2 [leverage])
    //
    // 1_800 [fee] = 300 [total contracts] * (1 / 50_001 [closing price]) * 0.0030 [fee
    // coefficient]
    //
    // 1_044_269 [new reserve] = 1_146_071 [reserve] + 100_002 [closed margin] - 199_996 [new
    // margin] - 1_800 [fee] - 8 [pnl]
    let expected_off_chain_balance = 1_044_269;
    wait_until_balance_equals(&test, expected_off_chain_balance).await;

    // The direction changing order is split into two trades: one to close the short position and
    // another to open the long position.

    // Close short position.
    //
    // 99_394 [1st trade amount] = 100_002 [closed margin] - 600 [fee] - 8 [pnl]
    check_trade(
        &test,
        &order_id,
        Direction::Long,
        100,
        Some(-8),
        600,
        99_394,
    );

    // Open long position, threfore no PNL.
    //
    // -201_196 [2nd trade amount] = -199_996 [new margin] - 1_200 [fee]
    check_trade(
        &test,
        &order_id,
        Direction::Long,
        200,
        None,
        1_200,
        -201_196,
    );

    tracing::info!(
        app_off_chain_balance = %expected_off_chain_balance,
        position = ?test.app.rx.position(),
        "Changed position direction"
    );
}

async fn wait_until_position_matches(test: &TestSetup, contracts: u64, direction: Direction) {
    wait_until!(matches!(
        test.app
            .rx
            .position()
            .map(|p| p.quantity == contracts as f32 && p.direction == direction),
        Some(true)
    ));
}

async fn wait_until_balance_equals(test: &TestSetup, target: u64) {
    wait_until!(
        target
            == test
                .app
                .rx
                .wallet_info()
                .unwrap()
                .balances
                .off_chain
                .unwrap()
    );
}

#[track_caller]
fn check_trade(
    test: &TestSetup,
    order_id: &str,
    direction: Direction,
    contracts: u64,
    pnl: Option<i64>,
    fee_sat: u64,
    // Positive if the trader received coins; negative if the trader paid coins.
    amount_sat: i64,
) {
    let can_find_trade = test
        .app
        .rx
        .wallet_info()
        .unwrap()
        .history
        .iter()
        .any(|item| match &item.wallet_type {
            WalletHistoryItemType::Trade {
                order_id: trade_order_id,
                fee_sat: trade_fee_sat,
                pnl: trade_pnl,
                contracts: trade_contracts,
                direction: trade_direction,
            } => {
                if trade_order_id == order_id {
                    tracing::debug!(?item, "Checking trade values");

                    let relative_amount_sat = relative_amount_sat(item.amount_sats, item.flow);

                    *trade_fee_sat == fee_sat
                        && trade_pnl == &pnl
                        && *trade_contracts == contracts
                        && trade_direction == &direction.to_string()
                        && relative_amount_sat == amount_sat
                } else {
                    false
                }
            }
            _ => false,
        });

    assert!(can_find_trade)
}

fn relative_amount_sat(amount_sat: u64, flow: PaymentFlow) -> i64 {
    let amount_sat = amount_sat as i64;

    match flow {
        PaymentFlow::Inbound => amount_sat,
        PaymentFlow::Outbound => -amount_sat,
    }
}
