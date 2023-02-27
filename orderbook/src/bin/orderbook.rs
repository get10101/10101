use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use orderbook::routes::router;
use orderbook::run_migration;
use std::net::SocketAddr;
use tracing::level_filters::LevelFilter;

#[tokio::main]
async fn main() {
    orderbook::logger::init_tracing(LevelFilter::DEBUG, false).unwrap();

    // set up database connection pool
    let conn_spec = "postgres://postgres:mysecretpassword@localhost:5432/orderbook".to_string();
    let manager = ConnectionManager::<PgConnection>::new(conn_spec);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().unwrap();
    run_migration(&mut conn);

    let app = router(pool);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
