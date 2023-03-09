use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use coordinator::cli::Opts;
use coordinator::logger;
use coordinator::node::Node;
use coordinator::routes::router;
use coordinator::run_migration;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use ln_dlc_node::seed::Bip39Seed;
use rand::thread_rng;
use rand::RngCore;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::metadata::LevelFilter;

const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";
const PROCESS_TRADE_REQUESTS_INTERVAL: Duration = Duration::from_secs(30);

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
        ln_dlc_node::node::Node::new_coordinator(
            "coordinator",
            network,
            data_dir.as_path(),
            address,
            opts.p2p_announcement_addresses(),
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

    let pending_trades = Arc::new(Mutex::new(HashSet::<PublicKey>::new()));

    tokio::spawn({
        let node = node.clone();
        let pending_trades = pending_trades.clone();
        async move {
            loop {
                tokio::time::sleep(PROCESS_TRADE_REQUESTS_INTERVAL).await;

                // TODO: Remove pending trades if we were unable to fulfill them within a
                // certain time

                let mut pending_trades = match pending_trades.lock() {
                    Ok(pending_trades) => pending_trades,
                    Err(e) => {
                        tracing::warn!("Failed to lock pending trades: {e:#}");
                        continue;
                    }
                };

                let mut to_be_deleted: HashSet<PublicKey> = HashSet::new();

                for pk in pending_trades.iter() {
                    let peer = pk.to_string();

                    tracing::debug!(
                        %peer,
                        "Checking for DLC offers"
                    );

                    let sub_channel = match node.get_sub_channel_offer(pk) {
                        Ok(Some(sub_channel)) => sub_channel,
                        Ok(None) => {
                            tracing::debug!(
                                %peer,
                                "No DLC channel offers found"
                            );
                            continue;
                        }
                        Err(e) => {
                            tracing::error!(
                                peer = %pk.to_string(),
                                "Unable to retrieve DLC channel offer: {e:#}"
                            );
                            continue;
                        }
                    };

                    tracing::info!(%peer, "Found DLC channel offer");

                    to_be_deleted.insert(*pk);

                    let channel_id = sub_channel.channel_id;

                    tracing::info!(
                        %peer,
                        channel_id = %hex::encode(channel_id),
                        "Accepting DLC channel offer"
                    );

                    if let Err(e) = node.accept_dlc_channel_offer(&channel_id) {
                        tracing::error!(
                            channel_id = %hex::encode(channel_id),
                            "Failed to accept subchannel: {e:#}"
                        );
                    };
                }

                for delete_me in to_be_deleted {
                    pending_trades.remove(&delete_me);
                }
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

    let app = router(
        Node {
            inner: node,
            pending_trades,
        },
        pool,
    );

    tracing::debug!("listening on http://{}", http_address);
    axum::Server::bind(&http_address)
        .serve(app.into_make_service())
        .await?;

    tracing::trace!("Server has had been launched");

    Ok(())
}
