use crate::models::Order;
use crate::routes::router;
use crate::routes::NewOrder;
use crate::routes::UpdateOrder;
use crate::run_migration;
use crate::tests::start_postgres;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use reqwest::Url;
use std::net::SocketAddr;
use std::net::TcpListener;
use testcontainers::clients::Cli;

#[tokio::test]
async fn crud_test() {
    tracing_subscriber::fmt()
        .with_env_filter("debug,actix_web=debug")
        .with_test_writer()
        .init();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    // let conn_spec = "postgres://postgres:mysecretpassword@localhost:5432/orderbook".to_string();
    let manager = ConnectionManager::<PgConnection>::new(conn_spec);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().unwrap();
    run_migration(&mut conn);

    let listener = TcpListener::bind("127.0.0.1:8000".parse::<SocketAddr>().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(router(pool).into_make_service())
            .await
            .unwrap();
    });

    let client = reqwest::Client::new();

    let url = format!("http://{addr}");
    let url = Url::parse(url.as_str()).unwrap();
    let all_orders_url = url.join("orders").unwrap();
    let order_url = url.join("orders").unwrap();

    let orders: Vec<Order> = client
        .get(all_orders_url.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(orders.is_empty());

    let order: Order = client
        .post(order_url.clone())
        .json(&NewOrder {
            price: 20000,
            maker_id: "Bob the Maker".to_string(),
            taken: false,
        })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(order.id > 0);

    let update_order_url = url
        .join(format!("orders/{}", order.id.to_string().as_str()).as_str())
        .unwrap();

    dbg!(&update_order_url.to_string());

    let order: Order = client
        .put(update_order_url.clone())
        .json(&UpdateOrder { taken: true })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(order.taken);

    let deleted: i32 = client
        .delete(update_order_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(deleted, 1);

    let orders: Vec<Order> = client
        .get(all_orders_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(orders.is_empty());
}
