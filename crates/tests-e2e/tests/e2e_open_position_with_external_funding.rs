use bitcoin::Amount;
use native::api;
use native::api::ContractSymbol;
use native::health::Service;
use native::health::ServiceStatus;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use std::time::Duration;
use tests_e2e::app::submit_unfunded_channel_opening_order;
use tests_e2e::http::init_reqwest;
use tests_e2e::lnd_mock::LndMock;
use tests_e2e::setup::TestSetup;
use tests_e2e::wait_until;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_open_position_with_external_lightning_funding() {
    let test = TestSetup::new().await;
    test.fund_coordinator(Amount::ONE_BTC, 2).await;
    let app = &test.app;

    let order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 1.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    submit_unfunded_channel_opening_order(order.clone(), 10_000, 10_000, 5_000, 1_000).unwrap();

    let client = init_reqwest();
    let lnd_mock = LndMock::new_local(client.clone());

    // wait for the watchers before paying the invoice.
    tokio::time::sleep(Duration::from_secs(1)).await;
    tracing::info!("Paying invoice");
    lnd_mock.pay_invoice().await.unwrap();

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
