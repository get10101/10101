use anyhow::Context;
use anyhow::Result;
use coordinator::cli::Opts;
use coordinator::db;
use coordinator::logger;
use coordinator::node;
use coordinator::node::Node;
use coordinator::node::TradeAction;
use coordinator::position::models::Position;
use coordinator::position::models::PositionState;
use coordinator::routes::router;
use coordinator::run_migration;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::node::PaymentMap;
use ln_dlc_node::seed::Bip39Seed;
use rand::thread_rng;
use rand::RngCore;
use rust_decimal::Decimal;
use std::backtrace::Backtrace;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tracing::metadata::LevelFilter;

const PROCESS_INCOMING_MESSAGES_INTERVAL: Duration = Duration::from_secs(5);

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

    logger::init_tracing(LevelFilter::DEBUG, false)?;

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
        ln_dlc_node::node::Node::new_coordinator(
            "coordinator",
            network,
            data_dir.as_path(),
            PaymentMap::default(),
            address,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
            opts.p2p_announcement_addresses(),
            opts.electrum,
            seed,
            ephemeral_randomness,
        )
        .await?,
    );

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                if let Err(e) = node.sync() {
                    tracing::error!("Failed to sync node. Error: {e:#}");
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    });

    {
        let dlc_manager = node.dlc_manager.clone();
        let sub_channel_manager = node.sub_channel_manager.clone();
        tokio::spawn({
            let dlc_message_handler = node.dlc_message_handler.clone();
            let peer_manager = node.peer_manager.clone();

            async move {
                loop {
                    if let Err(e) = node::process_incoming_messages_internal(
                        &dlc_message_handler,
                        &dlc_manager,
                        &sub_channel_manager,
                        &peer_manager,
                    ) {
                        tracing::error!("Unable to process internal message: {e:#}");
                    }

                    tokio::time::sleep(PROCESS_INCOMING_MESSAGES_INTERVAL).await;
                }
            }
        })
    };

    // set up database connection pool
    let manager = ConnectionManager::<PgConnection>::new(opts.database);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let mut conn = pool.get().unwrap();
    run_migration(&mut conn);

    let node = Node {
        inner: node,
        pool: pool.clone(),
    };

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await;

                let mut conn = node.pool.get().unwrap();
                match db::positions::Position::get_all_open_positions(&mut conn) {
                    Ok(positions) => {
                        let positions = positions
                            .into_iter()
                            .filter(|p| {
                                p.position_state == PositionState::Open
                                    && OffsetDateTime::now_utc().ge(&p.expiry_timestamp)
                            })
                            .collect::<Vec<Position>>();

                        for position in positions.iter() {
                            tracing::debug!(%position.expiry_timestamp, "Attempting to closed expired position with {}", position.trader);

                            if !node.is_connected(&position.trader) {
                                tracing::info!("Could not close expired position with {} as trader is not connected.", position.trader);
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

                            // todo: fetch closing price from bitmex
                            let closing_price = Decimal::from(30000);

                            match node
                                .close_position(position, closing_price, channel_id)
                                .await
                            {
                                Ok(_) => tracing::info!(
                                    "Successfully closed expired position with {}",
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
                    Err(e) => {
                        tracing::error!("Failed to get positions. Error: {e:?}");
                    }
                }
                tokio::time::sleep(Duration::from_secs(600)).await;
            }
        }
    });

    let app = router(node, pool);

    tracing::debug!("listening on http://{}", http_address);
    axum::Server::bind(&http_address)
        .serve(app.into_make_service())
        .await?;

    tracing::trace!("Server has had been launched");

    Ok(())
}
