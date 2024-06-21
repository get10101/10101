use crate::logger::init_tracing_for_test;
use crate::orderbook::db::orders;
use crate::orderbook::tests::setup_db;
use crate::orderbook::tests::start_postgres;
use bitcoin::secp256k1::PublicKey;
use rust_decimal_macros::dec;
use std::str::FromStr;
use testcontainers::clients::Cli;
use time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;
use xxi_node::commons;
use xxi_node::commons::Direction;
use xxi_node::commons::NewLimitOrder;
use xxi_node::commons::NewMarketOrder;
use xxi_node::commons::OrderReason;
use xxi_node::commons::OrderState;

#[tokio::test]
async fn crud_test() {
    init_tracing_for_test();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let order = orders::insert_limit_order(
        &mut conn,
        dummy_limit_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
        OrderReason::Manual,
    )
    .unwrap();

    assert_eq!(order.order_state, OrderState::Open);
}

#[tokio::test]
async fn test_all_limit_orders() {
    init_tracing_for_test();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all_limit_orders(&mut conn).unwrap();
    assert!(orders.is_empty());

    let order_1 = dummy_limit_order(OffsetDateTime::now_utc() + Duration::minutes(1));
    orders::insert_limit_order(&mut conn, order_1, OrderReason::Manual).unwrap();

    let order_2 = dummy_market_order(OffsetDateTime::now_utc() + Duration::minutes(1));
    orders::insert_market_order(&mut conn, order_2, OrderReason::Manual).unwrap();

    let order_3 = dummy_limit_order(OffsetDateTime::now_utc() + Duration::minutes(1));
    let second_limit_order =
        orders::insert_limit_order(&mut conn, order_3, OrderReason::Manual).unwrap();
    orders::set_order_state(&mut conn, second_limit_order.id, OrderState::Failed).unwrap();

    let orders = orders::all_limit_orders(&mut conn).unwrap();
    assert_eq!(orders.len(), 1);
}

fn dummy_market_order(expiry: OffsetDateTime) -> NewMarketOrder {
    NewMarketOrder {
        id: Uuid::new_v4(),
        trader_id: PublicKey::from_str(
            "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
        )
        .unwrap(),
        direction: Direction::Long,
        quantity: dec!(100.0),
        expiry,
        contract_symbol: commons::ContractSymbol::BtcUsd,
        leverage: dec!(1.0),
        stable: false,
    }
}

fn dummy_limit_order(expiry: OffsetDateTime) -> NewLimitOrder {
    NewLimitOrder {
        id: Uuid::new_v4(),
        price: dec!(20000.00),
        trader_id: PublicKey::from_str(
            "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
        )
        .unwrap(),
        direction: Direction::Long,
        quantity: dec!(100.0),
        expiry,
        contract_symbol: commons::ContractSymbol::BtcUsd,
        leverage: dec!(1.0),
        stable: false,
    }
}
