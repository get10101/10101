use crate::orderbook::db::orders;
use crate::orderbook::tests::setup_db;
use crate::orderbook::tests::start_postgres;
use bitcoin::secp256k1::PublicKey;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderType;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::sync::Once;
use testcontainers::clients::Cli;
use time::Duration;
use time::OffsetDateTime;
use trade::Direction;
use uuid::Uuid;

fn init_tracing() {
    static TRACING_TEST_SUBSCRIBER: Once = Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .init()
    })
}

#[tokio::test]
async fn crud_test() {
    init_tracing();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all(&mut conn, true).unwrap();
    assert!(orders.is_empty());

    let order = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
    )
    .unwrap();

    let order = orders::taken(&mut conn, order.id, true).unwrap();
    assert!(order.taken);

    let deleted = orders::delete_with_id(&mut conn, order.id).unwrap();
    assert_eq!(deleted, 1);

    let orders = orders::all(&mut conn, true).unwrap();
    assert!(orders.is_empty());
}

#[tokio::test]
async fn test_filter_expired_orders() {
    init_tracing();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all(&mut conn, true).unwrap();
    assert!(orders.is_empty());

    let order = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() + Duration::minutes(1)),
    )
    .unwrap();
    let _ = orders::insert(
        &mut conn,
        dummy_order(OffsetDateTime::now_utc() - Duration::minutes(1)),
    )
    .unwrap();

    let orders = orders::all(&mut conn, false).unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders.get(0).unwrap().id, order.id);

    let orders = orders::all(&mut conn, true).unwrap();
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
    }
}
