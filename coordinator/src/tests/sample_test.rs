use crate::orderbook::models::NewOrder;
use crate::orderbook::models::Order;
use crate::run_migration;
use crate::tests::start_postgres;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use testcontainers::clients::Cli;

#[tokio::test]
async fn crud_test() {
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .init();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let manager = ConnectionManager::<PgConnection>::new(conn_spec);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().unwrap();
    run_migration(&mut conn);

    let orders = Order::all(&mut conn).unwrap();
    assert!(orders.is_empty());

    let order = Order::insert(
        &mut conn,
        NewOrder {
            price: 20000,
            maker_id: "Bob the Maker".to_string(),
            taken: false,
        },
    )
    .unwrap();
    assert!(order.id > 0);

    let order = Order::update(&mut conn, order.id, true).unwrap();
    assert!(order.taken);

    let deleted = Order::delete_with_id(&mut conn, order.id).unwrap();
    assert_eq!(deleted, 1);

    let orders = Order::all(&mut conn).unwrap();
    assert!(orders.is_empty());
}
