mod registration_test;
mod sample_test;

use crate::run_migration;
use anyhow::Result;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use std::sync::Once;
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::images;
use testcontainers::images::generic::GenericImage;
use testcontainers::Container;

pub fn init_tracing() {
    static TRACING_TEST_SUBSCRIBER: Once = Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .init()
    })
}

pub fn start_postgres(docker: &Cli) -> Result<(Container<GenericImage>, String)> {
    let db = "postgres-db-test";
    let user = "postgres-user-test";
    let password = "postgres-password-test";

    let postgres = images::generic::GenericImage::new("postgres", "15-alpine")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", db)
        .with_env_var("POSTGRES_USER", user)
        .with_env_var("POSTGRES_PASSWORD", password);

    let node = docker.run(postgres);

    let connection_string = &format!(
        "postgres://{}:{}@127.0.0.1:{}/{}",
        user,
        password,
        node.get_host_port_ipv4(5432),
        db
    );

    Ok((node, connection_string.clone()))
}

pub fn setup_db(db_url: String) -> PooledConnection<ConnectionManager<PgConnection>> {
    let manager = ConnectionManager::<PgConnection>::new(db_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().unwrap();
    run_migration(&mut conn);
    conn
}
