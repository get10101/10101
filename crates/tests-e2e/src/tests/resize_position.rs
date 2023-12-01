use crate::setup::TestSetup;
use crate::wait_until;
use native::api;
use native::api::ContractSymbol;
use native::api::Direction;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use tokio::task::spawn_blocking;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_resize_position() {
    let test = TestSetup::new_after_funding().await;
    let app = &test.app;

    // Buy 10 => 10 Long.

    let opening_market_order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        quantity: 10.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = opening_market_order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    // Wait until the position has been created.
    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Open && pos.quantity == 10.0)
        .unwrap_or(false));

    // Sell 5 => 5 Long.

    let resize_market_order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Short,
        quantity: 5.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = resize_market_order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Resizing)
        .unwrap_or(false));

    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Open && pos.quantity == 5.0)
        .unwrap_or(false));

    let position = app.rx.position().unwrap();

    assert_eq!(position.direction, Direction::Long);
    assert_eq!(position.contract_symbol, ContractSymbol::BtcUsd);
    assert_eq!(position.leverage, 2.0);

    // Sell 10 => 5 Short.

    let resize_market_order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Short,
        quantity: 10.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = resize_market_order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Resizing)
        .unwrap_or(false));

    // Wait until the position has been resized.
    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Open && pos.direction == Direction::Short)
        .unwrap_or(false));

    let position = app.rx.position().unwrap();

    assert_eq!(position.quantity, 5.0);
    assert_eq!(position.contract_symbol, ContractSymbol::BtcUsd);
    assert_eq!(position.leverage, 2.0);

    // Sell 5 => 10 Short.

    let resize_market_order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Short,
        quantity: 5.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = resize_market_order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Resizing)
        .unwrap_or(false));

    // Wait until the position has been resized.
    wait_until!(app
        .rx
        .position()
        .map(|pos| pos.position_state == PositionState::Open && pos.quantity == 10.0)
        .unwrap_or(false));

    let position = app.rx.position().unwrap();

    assert_eq!(position.direction, Direction::Short);
    assert_eq!(position.contract_symbol, ContractSymbol::BtcUsd);
    assert_eq!(position.leverage, 2.0);

    // Buy 10 => 0 contracts.

    let resize_market_order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        quantity: 10.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = resize_market_order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    wait_until!(app.rx.position().unwrap().position_state == PositionState::Closing);
    wait_until!(app.rx.position_close().is_some());
}
