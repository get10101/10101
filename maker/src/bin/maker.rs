use anyhow::Context;
use anyhow::Result;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::PaymentMap;
use ln_dlc_node::seed::Bip39Seed;
use maker::cli::Opts;
use maker::logger;
use maker::routes::router;
use maker::run_migration;
use maker::trading;
use rand::thread_rng;
use rand::RngCore;
use std::backtrace::Backtrace;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tracing::metadata::LevelFilter;

const NODE_SYNC_INTERVAL: Duration = Duration::from_secs(20);

#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(
        #[allow(clippy::print_stderr)]
        Box::new(|info| {
            let backtrace = Backtrace::force_capture();

            tracing::error!(%info, "Aborting after panic in task");
            eprintln!("{backtrace}");

            std::process::abort()
        }),
    );

    let opts = Opts::read();
    let data_dir = opts.data_dir()?;
    let address = opts.p2p_address;
    let http_address = opts.http_address;
    let network = opts.network();

    logger::init_tracing(LevelFilter::DEBUG, opts.json)?;

    let mut ephemeral_randomness = [0; 32];
    thread_rng().fill_bytes(&mut ephemeral_randomness);

    let data_dir = data_dir.join(network.to_string());
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)
            .context(format!("Could not create data dir for {network}"))?;
    }

    let data_dir_string = data_dir.clone().into_os_string();
    tracing::info!("Data-dir: {data_dir_string:?}");

    let seed_path = data_dir.join("seed");
    let seed = Bip39Seed::initialize(&seed_path)?;

    let node = Arc::new(
        Node::new_app(
            "maker",
            network,
            data_dir.as_path(),
            PaymentMap::default(),
            address,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
            opts.esplora,
            seed,
            ephemeral_randomness,
        )
        .await?,
    );

    // TODO: We should move the sync into the node
    let wallet = node.wallet();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                loop {
                    let now = Instant::now();
                    match wallet.sync().await {
                        Ok(()) => tracing::info!(
                            "Background sync of on-chain wallet finished in {}ms.",
                            now.elapsed().as_millis()
                        ),
                        Err(err) => {
                            tracing::error!("Background sync of on-chain wallet failed: {}", err)
                        }
                    }
                    tokio::time::sleep(NODE_SYNC_INTERVAL).await;
                }
            });
    });

    let node_pubkey = node.info.pubkey;
    tokio::spawn(async move {
        match trading::run(opts.orderbook, node_pubkey, network).await {
            Ok(_) => {
                // all good
            }
            Err(error) => {
                tracing::error!("Trading logic died {error:#}")
            }
        }
    });

    // TODO: Process DLC message

    // set up database connection pool
    let manager = ConnectionManager::<PgConnection>::new(opts.database);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().expect("to get connection from pool");
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
