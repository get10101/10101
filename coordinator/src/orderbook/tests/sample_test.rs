use crate::logger::init_tracing_for_test;
use crate::orderbook::db::orders;
use crate::orderbook::tests::setup_db;
use crate::orderbook::tests::start_postgres;
use bitcoin::secp256k1::PublicKey;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderReason;
use orderbook_commons::OrderState;
use orderbook_commons::OrderType;
use rust_decimal_macros::dec;
use std::str::FromStr;
use testcontainers::clients::Cli;
use time::Duration;
use time::OffsetDateTime;
use trade::Direction;
use uuid::Uuid;

#[tokio::test]
async fn crud_test() {
    init_tracing_for_test();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all(&mut conn, true, true).unwrap();
    assert!(orders.is_empty());

    let order = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
        OrderReason::Manual,
    )
    .unwrap();

    let order = orders::set_is_taken(&mut conn, order.id, true).unwrap();
    assert_eq!(order.order_state, OrderState::Taken);
}

#[tokio::test]
async fn test_filter_expired_orders() {
    init_tracing_for_test();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all(&mut conn, true, true).unwrap();
    assert!(orders.is_empty());

    let order = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
        OrderReason::Manual,
    )
    .unwrap();
    let _ = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() - Duration::minutes(1)),
        OrderReason::Manual,
    )
    .unwrap();

    let orders = orders::all(&mut conn, false, true).unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders.get(0).unwrap().id, order.id);

    let orders = orders::all(&mut conn, true, true).unwrap();
    assert_eq!(orders.len(), 2);
}

#[tokio::test]
async fn test_filter_failed_orders() {
    init_tracing_for_test();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all(&mut conn, true, true).unwrap();
    assert!(orders.is_empty());

    let first_order = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
        OrderReason::Manual,
    )
    .unwrap();
    let second_order = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
        OrderReason::Manual,
    )
    .unwrap();
    orders::set_order_state(&mut conn, second_order.id, OrderState::Failed).unwrap();

    let orders = orders::all(&mut conn, false, false).unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders.get(0).unwrap().id, first_order.id);

    let orders = orders::all(&mut conn, false, true).unwrap();
    assert_eq!(orders.len(), 2);
}

fn dummy_order(expiry: OffsetDateTime) -> NewOrder {
    NewOrder {
        id: Uuid::new_v4(),
        price: dec!(20000.00),
        trader_id: PublicKey::from_str(
            "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
        )
        .unwrap(),
        direction: Direction::Long,
        quantity: dec!(100.0),
        order_type: OrderType::Market,
        expiry,
        contract_symbol: trade::ContractSymbol::BtcUsd,
        leverage: 1.0,
        stable: false,
    }
}
