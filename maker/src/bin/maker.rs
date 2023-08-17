use anyhow::Context;
use anyhow::Result;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::seed::Bip39Seed;
use maker::cli::Opts;
use maker::ln::ldk_config;
use maker::ln::EventHandler;
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
use tracing::metadata::LevelFilter;

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

    let node = Arc::new(ln_dlc_node::node::Node::new(
        ldk_config(),
        ln_dlc_node::scorer::persistent_scorer,
        "maker",
        network,
        data_dir.as_path(),
        Arc::new(InMemoryStore::default()),
        address,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
        ln_dlc_node::util::into_net_addresses(address),
        opts.esplora.clone(),
        seed,
        ephemeral_randomness,
        LnDlcNodeSettings::default(),
        opts.get_oracle_info().into(),
    )?);

    let event_handler = EventHandler::new(node.clone());
    let _running_node = node.start(event_handler)?;

    let node_pubkey = node.info.pubkey;

    let position_expiry = opts.position_expiry.unwrap_or(orderbook_commons::default_position_expiry());

    tokio::spawn(async move {
        match trading::run(
            &opts.orderbook,
            node_pubkey,
            network,
            opts.concurrent_orders,
            time::Duration::seconds(opts.order_expiry_after_seconds as i64),
        )
        .await
        {
            Ok(()) => {
                tracing::error!("Maker stopped trading");
            }
            Err(error) => {
                tracing::error!("Maker stopped trading: {error:#}");
            }
        }
    });

    let manager = ConnectionManager::<PgConnection>::new(opts.database);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().expect("to get connection from pool");
    run_migration(&mut conn);

    let app = router(node, pool);

    let addr = SocketAddr::from((http_address.ip(), http_address.port()));
    tracing::debug!("Listening on http://{}", addr);

    match axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
    {
        Ok(_) => {
            tracing::info!("HTTP server stopped running");
        }
        Err(e) => {
            tracing::error!("HTTP server stopped running: {e:#}");
        }
    }

    Ok(())
}
