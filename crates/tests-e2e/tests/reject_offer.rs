use bitcoin::Amount;
use native::api;
use native::api::ContractSymbol;
use native::health::Service;
use native::health::ServiceStatus;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::order::OrderState;
use native::trade::position::PositionState;
use tests_e2e::setup::TestSetup;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn reject_offer() {
    let test = TestSetup::new().await;
    test.fund_coordinator(Amount::ONE_BTC).await;
    test.fund_app(Amount::from_sat(250_000)).await;

    let app = &test.app;

    let invalid_order = NewOrder {
        leverage: 1.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 2000.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    // submit order for which the app does not have enough liquidity. will fail with `Failed to
    // accept dlc channel offer. Invalid state: Not enough UTXOs for amount`
    spawn_blocking({
        let order = invalid_order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    assert_eq!(app.rx.status(Service::Orderbook), ServiceStatus::Online);
    assert_eq!(app.rx.status(Service::Coordinator), ServiceStatus::Online);

    // Assert that the order was posted
    wait_until!(app.rx.order().is_some());
    assert_eq!(app.rx.order().unwrap().quantity, invalid_order.quantity);
    assert_eq!(app.rx.order().unwrap().direction, invalid_order.direction);
    assert_eq!(
        app.rx.order().unwrap().contract_symbol,
        invalid_order.contract_symbol
    );
    assert_eq!(app.rx.order().unwrap().leverage, invalid_order.leverage);

    // Assert that the order failed
    wait_until!(matches!(
        app.rx.order().unwrap().state,
        OrderState::Failed { .. }
    ));

    // Assert that no position has been opened
    wait_until!(app.rx.position().is_none());

    // Retry with a smaller order
    let order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 100.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    spawn_blocking({
        let order = order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    // Assert that the order was posted
    wait_until!(app.rx.order().is_some());
    assert_eq!(app.rx.order().unwrap().quantity, order.quantity);
    assert_eq!(app.rx.order().unwrap().direction, order.direction);
    assert_eq!(
        app.rx.order().unwrap().contract_symbol,
        order.contract_symbol
    );
    assert_eq!(app.rx.order().unwrap().leverage, order.leverage);

    // Assert that the position is opened in the app
    wait_until!(app.rx.position().is_some());
    assert_eq!(app.rx.position().unwrap().quantity, order.quantity);
    assert_eq!(app.rx.position().unwrap().direction, order.direction);
    assert_eq!(
        app.rx.position().unwrap().contract_symbol,
        order.contract_symbol
    );
    assert_eq!(app.rx.position().unwrap().leverage, order.leverage);
    wait_until!(app.rx.position().unwrap().position_state == PositionState::Open);

    // TODO(holzeis): Add reject tests for SettleOffer and RenewOffer.
    // Unfortunately its not easy to provoke a reject for a settle offer or renew offer from a grey
    // box integration test.
}
