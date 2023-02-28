mod sample_test;

use anyhow::Result;
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::images;
use testcontainers::images::generic::GenericImage;
use testcontainers::Container;

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
