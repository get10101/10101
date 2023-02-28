use anyhow::Context;
use anyhow::Result;
use bitcoin::Network;
use coordinator::cli::Opts;
use coordinator::logger;
use coordinator::routes::router;
use coordinator::run_migration;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::Node;
use ln_dlc_node::seed::Bip39Seed;
use rand::thread_rng;
use rand::RngCore;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::metadata::LevelFilter;

const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::read();
    let data_dir = opts.data_dir()?;
    let address = opts.p2p_address;
    let http_address = opts.http_address;
    let network = Network::Regtest;

    logger::init_tracing(LevelFilter::DEBUG, false)?;

    let mut ephemeral_randomness = [0; 32];
    thread_rng().fill_bytes(&mut ephemeral_randomness);

    let data_dir = data_dir.join(network.to_string());
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)
            .context(format!("Could not create data dir for {network}"))?;
    }

    let seed_path = data_dir.join("seed");
    let seed = Bip39Seed::initialize(&seed_path)?;

    let node = Arc::new(
        Node::new_coordinator(
            "coordinator".to_string(),
            network,
            data_dir.as_path(),
            address,
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
        )
        .await,
    );

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                // todo: the node sync should not swallow the error.
                node.sync();
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    });

    let background_processor = node.start().await.expect("background processor to start");
    tokio::spawn({
        async move {
            if let Err(err) = background_processor.join() {
                tracing::error!(?err, "Background processor stopped unexpected");
            }
        }
    });

    // set up database connection pool
    let conn_spec = "postgres://postgres:mysecretpassword@localhost:5432/orderbook".to_string();
    let manager = ConnectionManager::<PgConnection>::new(conn_spec);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().unwrap();
    run_migration(&mut conn);

    let app = router(node, pool);

    let addr = SocketAddr::from((http_address.ip(), http_address.port()));
    tracing::debug!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    tracing::trace!("Server has had been launched");

    Ok(())
}
