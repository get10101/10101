use anyhow::Context;
use anyhow::Result;
use coordinator::cli::Opts;
use coordinator::db;
use coordinator::logger;
use coordinator::metrics::init_meter;
use coordinator::metrics::CHANNEL_BALANCE_MSATOSHI;
use coordinator::metrics::CHANNEL_INBOUND_CAPACITY_MSATOSHI;
use coordinator::metrics::CHANNEL_IS_USABLE;
use coordinator::metrics::CHANNEL_OUTBOUND_CAPACITY_MSATOSHI;
use coordinator::metrics::CONNECTED_PEERS;
use coordinator::metrics::NODE_BALANCE_SATOSHI;
use coordinator::node::connection;
use coordinator::node::Node;
use coordinator::node::TradeAction;
use coordinator::position::models::Position;
use coordinator::position::models::PositionState;
use coordinator::routes::router;
use coordinator::run_migration;
use coordinator::settings::Settings;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::seed::Bip39Seed;
use opentelemetry::KeyValue;
use rand::thread_rng;
use rand::RngCore;
use std::backtrace::Backtrace;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::task::spawn_blocking;
use tracing::metadata::LevelFilter;
use trade::bitmex_client::BitmexClient;

const PROCESS_PROMETHEUS_METRICS: Duration = Duration::from_secs(10);
const PROCESS_INCOMING_DLC_MESSAGES_INTERVAL: Duration = Duration::from_secs(5);
const POSITION_SYNC_INTERVAL: Duration = Duration::from_secs(300);
const CONNECTION_CHECK_INTERVAL: Duration = Duration::from_secs(30);

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

    let settings = Settings::new(&data_dir).await;

    let node = Arc::new(ln_dlc_node::node::Node::new_coordinator(
        "10101.finance",
        network,
        data_dir.as_path(),
        InMemoryStore::default(),
        address,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
        opts.p2p_announcement_addresses(),
        opts.esplora.clone(),
        seed,
        ephemeral_randomness,
        settings.ln_dlc.clone(),
        opts.get_oracle_info(),
    )?);

    // set up database connection pool
    let manager = ConnectionManager::<PgConnection>::new(opts.database);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get()?;
    run_migration(&mut conn);

    let node = Node::new(node, pool.clone());
    node.update_settings(settings.as_node_settings()).await;

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
                spawn_blocking(move || {
                    let cx = opentelemetry::Context::current();
                    let channels = node.inner.channel_manager.list_channels();
                    for channel_detail in channels {
                        let key_values = [
                            KeyValue::new("channel_id", hex::encode(channel_detail.channel_id)),
                            KeyValue::new("is_outbound", channel_detail.is_outbound),
                            KeyValue::new("is_public", channel_detail.is_public),
                        ];
                        CHANNEL_BALANCE_MSATOSHI.observe(
                            &cx,
                            channel_detail.balance_msat,
                            &key_values,
                        );
                        CHANNEL_OUTBOUND_CAPACITY_MSATOSHI.observe(
                            &cx,
                            channel_detail.outbound_capacity_msat,
                            &key_values,
                        );
                        CHANNEL_INBOUND_CAPACITY_MSATOSHI.observe(
                            &cx,
                            channel_detail.inbound_capacity_msat,
                            &key_values,
                        );
                        CHANNEL_IS_USABLE.observe(
                            &cx,
                            channel_detail.is_usable as u64,
                            &key_values,
                        );
                    }

                    let connected_peers = node.inner.list_peers().len();
                    CONNECTED_PEERS.observe(&cx, connected_peers as u64, &[]);
                    let offchain = node.inner.get_ldk_balance();

                    NODE_BALANCE_SATOSHI.observe(
                        &cx,
                        offchain.available,
                        &[
                            KeyValue::new("type", "off-chain"),
                            KeyValue::new("status", "available"),
                        ],
                    );
                    NODE_BALANCE_SATOSHI.observe(
                        &cx,
                        offchain.pending_close,
                        &[
                            KeyValue::new("type", "off-chain"),
                            KeyValue::new("status", "pending_close"),
                        ],
                    );

                    match node.inner.get_on_chain_balance() {
                        Ok(onchain) => {
                            NODE_BALANCE_SATOSHI.observe(
                                &cx,
                                onchain.confirmed,
                                &[
                                    KeyValue::new("type", "on-chain"),
                                    KeyValue::new("status", "confirmed"),
                                ],
                            );
                            NODE_BALANCE_SATOSHI.observe(
                                &cx,
                                onchain.immature,
                                &[
                                    KeyValue::new("type", "on-chain"),
                                    KeyValue::new("status", "immature"),
                                ],
                            );
                            NODE_BALANCE_SATOSHI.observe(
                                &cx,
                                onchain.trusted_pending,
                                &[
                                    KeyValue::new("type", "on-chain"),
                                    KeyValue::new("status", "trusted_pending"),
                                ],
                            );
                            NODE_BALANCE_SATOSHI.observe(
                                &cx,
                                onchain.untrusted_pending,
                                &[
                                    KeyValue::new("type", "on-chain"),
                                    KeyValue::new("status", "untrusted_pending"),
                                ],
                            );
                        }
                        Err(err) => {
                            tracing::error!(
                                "Could not retrieve on-chain balance for metrics {err:#}"
                            )
                        }
                    }
                })
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
                tokio::time::sleep(POSITION_SYNC_INTERVAL).await;

                let mut conn = match node.pool.get() {
                    Ok(conn) => conn,
                    Err(e) => {
                        tracing::error!("Failed to get pool connection. Error: {e:?}");
                        continue;
                    }
                };

                let positions = match db::positions::Position::get_all_open_positions(&mut conn) {
                    Ok(positions) => positions,
                    Err(e) => {
                        tracing::error!("Failed to get positions. Error: {e:?}");
                        continue;
                    }
                };

                let positions = positions
                    .into_iter()
                    .filter(|p| {
                        p.position_state == PositionState::Open
                            && OffsetDateTime::now_utc().ge(&p.expiry_timestamp)
                    })
                    .collect::<Vec<Position>>();

                for position in positions.iter() {
                    tracing::trace!(trader_pk=%position.trader, %position.expiry_timestamp, "Attempting to close expired position");

                    if !node.is_connected(&position.trader) {
                        tracing::debug!(
                            "Could not close expired position with {} as trader is not connected.",
                            position.trader
                        );
                        continue;
                    }

                    let channel_id = match node.decide_trade_action(&position.trader) {
                        Ok(TradeAction::Close(channel_id)) => channel_id,
                        Ok(_) => {
                            tracing::error!(
                                ?position,
                                "Unable to find sub channel of expired position."
                            );
                            continue;
                        }
                        Err(e) => {
                            tracing::error!(
                                ?position,
                                "Failed to decide trade action. Error: {e:?}"
                            );
                            continue;
                        }
                    };

                    let closing_price =
                        match BitmexClient::get_quote(&position.expiry_timestamp).await {
                            Ok(quote) => match position.direction {
                                trade::Direction::Long => quote.bid_price,
                                trade::Direction::Short => quote.ask_price,
                            },
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to get quote from bitmex for {} at {}. Error: {e:?}",
                                    position.trader,
                                    position.expiry_timestamp
                                );
                                continue;
                            }
                        };

                    match node
                        .close_position(position, closing_price, channel_id)
                        .await
                    {
                        Ok(_) => tracing::info!(
                            "Successfully proposed to close expired position with {}",
                            position.trader
                        ),
                        Err(e) => tracing::warn!(
                            ?position,
                            "Failed to close expired position with {}. Error: {e:?}",
                            position.trader
                        ),
                    }
                }
            }
        }
    });

    tokio::spawn({
        let node = node.clone();
        connection::keep_public_channel_peers_connected(node.inner, CONNECTION_CHECK_INTERVAL)
    });

    let app = router(node, pool, settings, exporter);

    // Start the metrics exporter
    autometrics::prometheus_exporter::init();

    tracing::debug!("listening on http://{}", http_address);
    axum::Server::bind(&http_address)
        .serve(app.into_make_service())
        .await?;

    tracing::trace!("Server has had been launched");

    Ok(())
}
