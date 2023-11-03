use anyhow::Context;
use anyhow::Result;
use bitcoin::Network;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::seed::Bip39Seed;
use maker::cli::Opts;
use maker::health;
use maker::ln::ldk_config;
use maker::ln::EventHandler;
use maker::logger;
use maker::metrics;
use maker::metrics::init_meter;
use maker::orderbook_ws;
use maker::position;
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
use tokio::task::spawn_blocking;
use tracing::metadata::LevelFilter;

const PROCESS_PROMETHEUS_METRICS: Duration = Duration::from_secs(10);

/// Interval after which we'll try to reconnect to the pricefeed again
const PRICEFEED_RECONNECT_INTERVAL: Duration = Duration::from_secs(10);

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

    let exporter = init_meter();

    let opts = Opts::read();
    let data_dir = opts.data_dir()?;
    let address = opts.p2p_address;
    let http_address = opts.http_address;
    let network = opts.network();
    let bitmex_api_key = opts.bitmex_api_key.clone();
    let bitmex_api_secret = opts.bitmex_api_secret.clone();

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

    let announcement_addresses = ln_dlc_node::util::into_net_addresses(address);
    let node_alias = "maker";
    let node = Arc::new(ln_dlc_node::node::Node::new(
        ldk_config(),
        ln_dlc_node::scorer::persistent_scorer,
        node_alias,
        network,
        data_dir.as_path(),
        Arc::new(InMemoryStore::default()),
        address,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
        announcement_addresses.clone(),
        opts.esplora.clone(),
        seed,
        ephemeral_randomness,
        LnDlcNodeSettings::default(),
        opts.get_oracle_info().into(),
    )?);

    let event_handler = EventHandler::new(node.clone());
    let _running_node = node.start(event_handler, false)?;

    std::thread::spawn(node.sync_on_chain_wallet_periodically());

    let (health, health_tx) = health::Health::new();

    let bitmex_http_client = bitmex_client::client::Client::new(match network {
        Network::Bitcoin => bitmex_client::models::Network::Mainnet,
        _ => bitmex_client::models::Network::Testnet,
    });

    let bitmex_http_client = match (bitmex_api_key.clone(), bitmex_api_secret.clone()) {
        (Some(bitmex_api_key), Some(bitmex_secret)) => {
            tracing::info!("BitMEX credentials provided");
            bitmex_http_client.with_credentials(bitmex_api_key, bitmex_secret)
        }
        _ => {
            tracing::info!("BitMEX credentials not provided");
            bitmex_http_client
        }
    };

    let (position_manager, mailbox) = xtra::Mailbox::unbounded();
    tokio::spawn(xtra::run(
        mailbox,
        position::Manager::new(bitmex_http_client),
    ));

    let node_pubkey = node.info.pubkey;
    tokio::spawn({
        let orderbook_url = opts.orderbook.clone();
        let position_manager = position_manager.clone();
        async move {
            trading::run(
                &orderbook_url,
                node_pubkey,
                network,
                opts.concurrent_orders,
                time::Duration::seconds(opts.order_expiry_after_seconds as i64),
                health_tx.bitmex_pricefeed,
                position_manager,
                bitmex_api_key,
                bitmex_api_secret,
                PRICEFEED_RECONNECT_INTERVAL,
            )
            .await;
        }
    });

    let _monitor_coordinator_status = tokio::spawn({
        let endpoint = opts.orderbook.clone();
        let client = reqwest_client();
        let interval = Duration::from_secs(10);
        async move {
            health::check_health_endpoint(&client, endpoint, health_tx.coordinator, interval).await;
        }
    });

    let _collect_prometheus_metrics = tokio::spawn({
        let node = node.clone();
        let health = health.clone();
        async move {
            loop {
                let node = node.clone();
                let health = health.clone();
                spawn_blocking(move || metrics::collect(node, health))
                    .await
                    .expect("To spawn blocking thread");
                tokio::time::sleep(PROCESS_PROMETHEUS_METRICS).await;
            }
        }
    });

    let manager = ConnectionManager::<PgConnection>::new(opts.database);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().expect("to get connection from pool");
    run_migration(&mut conn);

    orderbook_ws::Client::new(
        opts.orderbook,
        node_pubkey,
        node.node_key(),
        position_manager.clone(),
        health_tx.orderbook,
    )
    .spawn_supervised_connection();

    let app = router(
        node,
        exporter,
        position_manager,
        health,
        announcement_addresses.clone(),
        node_alias,
    );

    // Start the metrics exporter
    autometrics::prometheus_exporter::init();

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
fn reqwest_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to build reqwest client")
}
