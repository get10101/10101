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

    tokio::spawn({
        let node = node.clone();
        async move {
            loop {
                tokio::time::sleep(PROCESS_TRADE_REQUESTS_INTERVAL).await;

                // todo: the coordinator pubkey should come from the cli arguments.
                let coordinator_pubkey =
                    "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9"
                        .parse()
                        .expect("hard coded pubkey to be valid");
                tracing::debug!(%coordinator_pubkey, "Checking for DLC offers");

                let sub_channel = match node.get_sub_channel_offer(&coordinator_pubkey) {
                    Ok(Some(sub_channel)) => sub_channel,
                    Ok(None) => {
                        tracing::debug!(%coordinator_pubkey, "No DLC channel offers found");
                        continue;
                    }
                    Err(e) => {
                        tracing::error!(peer = %coordinator_pubkey.to_string(), "Unable to retrieve DLC channel offer: {e:#}");
                        continue;
                    }
                };

                tracing::info!(%coordinator_pubkey, "Found DLC channel offer");

                let channel_id = sub_channel.channel_id;

                // todo: the maker should validate if the offered dlc channel matches it's submitted
                // order.

                tracing::info!(%coordinator_pubkey, channel_id = %hex::encode(channel_id), "Accepting DLC channel offer");

                if let Err(e) = node.accept_dlc_channel_offer(&channel_id) {
                    tracing::error!(channel_id = %hex::encode(channel_id), "Failed to accept subchannel: {e:#}");
                };
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
