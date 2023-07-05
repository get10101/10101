use native::api::ContractSymbol;
use native::api::{self};
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use tests_e2e::app::run_app;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::fund::fund_app_with_faucet;
use tests_e2e::http::init_reqwest;
use tests_e2e::tracing::init_tracing;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

fn dummy_order() -> NewOrder {
    NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 1.0,
        order_type: Box::new(OrderType::Market),
    }
}

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_open_position() {
    init_tracing();
    let client = init_reqwest();
    let coordinator = Coordinator::new_local(client.clone());
    assert!(coordinator.is_running().await);

    let app = run_app().await;

    fund_app_with_faucet(&client, 50_000).await.unwrap();
    wait_until!(app.rx.wallet_info().unwrap().balances.lightning == 50_000);

    let order = dummy_order();
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
}
