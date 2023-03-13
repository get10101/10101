use crate::orderbook::db::orders;
use crate::orderbook::routes::OrderType;
use crate::orderbook::tests::setup_db;
use crate::orderbook::tests::start_postgres;
use rust_decimal_macros::dec;
use testcontainers::clients::Cli;
use trade::Direction;

#[tokio::test]
async fn crud_test() {
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .init();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let orders = orders::all(&mut conn).unwrap();
    assert!(orders.is_empty());

    let order = orders::insert(
        &mut conn,
        crate::orderbook::routes::NewOrder {
            price: dec!(20000.00),
            trader_id: "Bob the Maker".to_string(),
            direction: Direction::Long,
            quantity: dec!(100.0),
            order_type: OrderType::Market,
        },
    )
    .unwrap();
    assert!(order.id > 0);

    let order = orders::update(&mut conn, order.id, true).unwrap();
    assert!(order.taken);

    let deleted = orders::delete_with_id(&mut conn, order.id).unwrap();
    assert_eq!(deleted, 1);

    let orders = orders::all(&mut conn).unwrap();
    assert!(orders.is_empty());
}
