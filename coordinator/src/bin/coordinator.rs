use anyhow::Context;
use anyhow::Result;
use bitcoin::key::XOnlyPublicKey;
use coordinator::backup::SledBackup;
use coordinator::cli::Opts;
use coordinator::dlc_handler;
use coordinator::dlc_handler::DlcHandler;
use coordinator::logger;
use coordinator::message::spawn_delivering_messages_to_authenticated_users;
use coordinator::message::NewUserMessage;
use coordinator::metrics;
use coordinator::metrics::init_meter;
use coordinator::node::expired_positions;
use coordinator::node::liquidated_positions;
use coordinator::node::rollover;
use coordinator::node::storage::NodeStorage;
use coordinator::node::unrealized_pnl;
use coordinator::node::Node;
use coordinator::notifications::NotificationService;
use coordinator::orderbook::async_match;
use coordinator::orderbook::collaborative_revert;
use coordinator::orderbook::trading;
use coordinator::routes::router;
use coordinator::run_migration;
use coordinator::scheduler::NotificationScheduler;
use coordinator::settings::Settings;
use coordinator::storage::CoordinatorTenTenOneStorage;
use coordinator::trade::websocket::InternalPositionUpdateMessage;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::event::NodeEventHandler;
use ln_dlc_node::seed::Bip39Seed;
use ln_dlc_node::CoordinatorEventHandler;
use ln_dlc_storage::DlcChannelEvent;
use rand::thread_rng;
use rand::RngCore;
use std::backtrace::Backtrace;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::spawn_blocking;
use tracing::metadata::LevelFilter;

const PROCESS_PROMETHEUS_METRICS: Duration = Duration::from_secs(10);
const PROCESS_INCOMING_DLC_MESSAGES_INTERVAL: Duration = Duration::from_millis(200);
const LIQUIDATED_POSITION_SYNC_INTERVAL: Duration = Duration::from_secs(30);
const EXPIRED_POSITION_SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);
const UNREALIZED_PNL_SYNC_INTERVAL: Duration = Duration::from_secs(10 * 60);

const NODE_ALIAS: &str = "10101.finance";

/// The prefix to the [`bdk_file_store`] database file where BDK persists
/// [`bdk::wallet::ChangeSet`]s.
///
/// We hard-code the prefix so that we can always be sure that we are loading the correct file on
/// start-up.
const WALLET_DB_PREFIX: &str = "10101-coordinator";

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

    logger::init_tracing(LevelFilter::DEBUG, opts.json, opts.tokio_console)?;

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

    let settings = Settings::new(&data_dir).await?;

    // set up database connection pool
    let manager = ConnectionManager::<PgConnection>::new(opts.database.clone());
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get()?;
    run_migration(&mut conn);

    let storage = CoordinatorTenTenOneStorage::new(data_dir.to_string_lossy().to_string());

    let node_storage = Arc::new(NodeStorage::new(pool.clone()));

    let node_event_handler = Arc::new(NodeEventHandler::new());

    let wallet_storage = bdk_file_store::Store::open_or_create_new(
        WALLET_DB_PREFIX.as_bytes(),
        data_dir.join("wallet"),
    )?;

    let (dlc_event_sender, dlc_event_receiver) = mpsc::channel::<DlcChannelEvent>();
    let node = Arc::new(ln_dlc_node::node::Node::new(
        ln_dlc_node::config::coordinator_config(),
        NODE_ALIAS,
        network,
        data_dir.as_path(),
        storage,
        node_storage,
        wallet_storage,
        address,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
        opts.electrs.clone(),
        seed,
        ephemeral_randomness,
        settings.ln_dlc.clone(),
        opts.get_oracle_infos()
            .into_iter()
            .map(|o| o.into())
            .collect(),
        XOnlyPublicKey::from_str(&opts.oracle_pubkey).expect("valid public key"),
        node_event_handler.clone(),
        dlc_event_sender,
    )?);

    let dlc_handler = DlcHandler::new(pool.clone(), node.clone());
    let _handle = dlc_handler::spawn_handling_outbound_dlc_messages(
        dlc_handler,
        node_event_handler.subscribe(),
    );

    let event_handler = CoordinatorEventHandler::new(node.clone(), None);
    let running = node.start(event_handler, dlc_event_receiver, false)?;

    // an internal channel to send updates about our position
    let (tx_position_feed, _rx) = broadcast::channel::<InternalPositionUpdateMessage>(100);

    let node = Node::new(
        node,
        running,
        pool.clone(),
        settings.to_node_settings(),
        tx_position_feed.clone(),
    );

    // TODO: Pass the tokio metrics into Prometheus
    if let Some(interval) = opts.tokio_metrics_interval_seconds {
        let handle = tokio::runtime::Handle::current();
        let runtime_monitor = tokio_metrics::RuntimeMonitor::new(&handle);
        let frequency = Duration::from_secs(interval);
        tokio::spawn(async move {
            for metrics in runtime_monitor.intervals() {
                tracing::debug!(?metrics, "tokio metrics");
                tokio::time::sleep(frequency).await;
            }
        });
    }

    tokio::spawn({
        let node = node.clone();

        // TODO: Do we still want to be able to update this at runtime?
        let interval = settings.ln_dlc.on_chain_sync_interval;
        async move {
            loop {
                if let Err(e) = node.inner.sync_on_chain_wallet().await {
                    tracing::info!("On-chain sync failed: {e:#}");
                }

                spawn_blocking({
                    let node = node.clone();
                    move || {
                        if let Err(e) = node.inner.dlc_manager.periodic_check() {
                            tracing::error!("Failed to run DLC manager periodic check: {e:#}");
                        }
                    }
                })
                .await
                .expect("task to complete");

                tokio::time::sleep(interval).await;
            }
        }
    });

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                let node = node.clone();
                spawn_blocking(move || node.process_incoming_dlc_messages())
                    .await
                    .expect("To spawn blocking thread");
                tokio::time::sleep(PROCESS_INCOMING_DLC_MESSAGES_INTERVAL).await;
            }
        }
    });

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                let node = node.clone();
                spawn_blocking(move || metrics::collect(node))
                    .await
                    .expect("To spawn blocking thread");
                tokio::time::sleep(PROCESS_PROMETHEUS_METRICS).await;
            }
        }
    });

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                tokio::time::sleep(UNREALIZED_PNL_SYNC_INTERVAL).await;
                if let Err(e) = unrealized_pnl::sync(node.clone()).await {
                    tracing::error!(
                        "Failed to sync unrealized PnL with positions in database: {e:#}"
                    );
                }
            }
        }
    });

    let (tx_user_feed, _rx) = broadcast::channel::<NewUserMessage>(100);

    let (tx_orderbook_feed, _rx) = broadcast::channel(100);

    let notification_service = NotificationService::new(opts.fcm_api_key.clone());

    let (_handle, auth_users_notifier) = spawn_delivering_messages_to_authenticated_users(
        pool.clone(),
        notification_service.get_sender(),
        tx_user_feed.clone(),
    );

    let (_handle, trading_sender) = trading::start(
        node.clone(),
        tx_orderbook_feed.clone(),
        auth_users_notifier.clone(),
        network,
        node.inner.oracle_pubkey,
    );
    let _handle = async_match::monitor(
        node.clone(),
        node_event_handler.subscribe(),
        auth_users_notifier.clone(),
        network,
        node.inner.oracle_pubkey,
    );
    let _handle = rollover::monitor(
        pool.clone(),
        node_event_handler.subscribe(),
        auth_users_notifier.clone(),
        network,
        node.clone(),
    );
    let _handle = collaborative_revert::monitor(
        pool.clone(),
        tx_user_feed.clone(),
        auth_users_notifier.clone(),
        network,
    );

    node.spawn_shadow_dlc_channels_task();
    node.spawn_watch_closing_channels();

    tokio::spawn({
        let node = node.clone();
        let trading_sender = trading_sender.clone();
        async move {
            loop {
                tokio::time::sleep(EXPIRED_POSITION_SYNC_INTERVAL).await;
                if let Err(e) = expired_positions::close(node.clone(), trading_sender.clone()).await
                {
                    tracing::error!("Failed to close expired positions! Error: {e:#}");
                }
            }
        }
    });

    tokio::spawn({
        let node = node.clone();
        let trading_sender = trading_sender.clone();
        async move {
            loop {
                tokio::time::sleep(LIQUIDATED_POSITION_SYNC_INTERVAL).await;
                liquidated_positions::monitor(node.clone(), trading_sender.clone()).await
            }
        }
    });

    let user_backup = SledBackup::new(data_dir.to_string_lossy().to_string());

    let app = router(
        node.clone(),
        pool.clone(),
        settings.clone(),
        exporter,
        opts.p2p_announcement_addresses(),
        NODE_ALIAS,
        trading_sender,
        tx_orderbook_feed,
        tx_position_feed,
        tx_user_feed,
        auth_users_notifier.clone(),
        notification_service.get_sender(),
        user_backup,
    );

    let sender = notification_service.get_sender();
    let notification_scheduler =
        NotificationScheduler::new(sender, settings, network, node, auth_users_notifier);
    tokio::spawn({
        let pool = pool.clone();
        let scheduler = notification_scheduler;
        async move {
            let scheduler = scheduler.await;
            scheduler
                .add_rollover_window_reminder_job(pool.clone())
                .await
                .expect("To add the rollover window reminder job");

            scheduler
                .add_rollover_window_close_reminder_job(pool.clone())
                .await
                .expect("To add the rollover window close reminder job");

            scheduler
                .add_reminder_to_close_expired_position_job(pool.clone())
                .await
                .expect("To add the close expired position reminder job");

            scheduler
                .add_reminder_to_close_liquidated_position_job(pool.clone())
                .await
                .expect("To add the close liquidated position reminder job");

            scheduler
                .start()
                .await
                .expect("to be able to start scheduler");
        }
    });

    tracing::debug!("Listening on http://{}", http_address);

    match axum::Server::bind(&http_address)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
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
