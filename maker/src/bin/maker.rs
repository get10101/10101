use anyhow::Context;
use anyhow::Result;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::Node;
use ln_dlc_node::seed::Bip39Seed;
use maker::cli::Opts;
use maker::logger;
use maker::routes::router;
use maker::run_migration;
use maker::trading;
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
    let network = opts.network();

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
        Node::new_app(
            "maker",
            network,
            data_dir.as_path(),
            address,
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
        )
        .await?,
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

    let node_info_string = node.info.to_string();
    tokio::spawn(async move {
        match trading::run(opts.orderbook, node_info_string, network).await {
            Ok(_) => {
                // all good
            }
            Err(error) => {
                tracing::error!("Trading logic died {error:#}")
            }
        }
    });

    // set up database connection pool
    let conn_spec = "postgres://postgres:mysecretpassword@localhost:5432/maker".to_string();
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
