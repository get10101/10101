use native::api;
use native::api::ContractSymbol;
use native::health::Service;
use native::health::ServiceStatus;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use tests_e2e::setup::TestSetup;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_open_position() {
    let test = TestSetup::new_after_funding().await;
    let app = &test.app;

    let order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 1.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };
    spawn_blocking({
        let order = order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    assert_eq!(app.rx.status(Service::Orderbook), ServiceStatus::Online);
    assert_eq!(app.rx.status(Service::Coordinator), ServiceStatus::Online);

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
}
