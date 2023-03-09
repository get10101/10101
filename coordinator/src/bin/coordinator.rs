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
use dlc_messages::SubChannelMessage;
use ln_dlc_node::seed::Bip39Seed;
use rand::thread_rng;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::metadata::LevelFilter;

const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";

// todo: This interval is quite arbitrary at the moment, come up with more sensible values
const PROCESS_INCOMING_MESSAGES_INTERVAL: Duration = Duration::from_secs(30);

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

    let pending_confirmations =
        Arc::new(Mutex::new(HashMap::<PublicKey, SubChannelMessage>::new()));
    tokio::spawn({
        let node = node.clone();
        let pending_confirmations = pending_confirmations.clone();
        async move {
            loop {
                match node.process_incoming_messages() {
                    Ok(confirm_messages) => {
                        for confirm_message in confirm_messages.iter() {
                            let message = confirm_message.1.clone();
                            let node_id = confirm_message.0;

                            if let Ok(mut pending_confirmations) = pending_confirmations.lock() {
                                pending_confirmations.insert(node_id, message);
                            } else {
                                tracing::warn!("Failed to acquire lock on pending confirmations");
                            }
                        }
                    }
                    Err(e) => tracing::error!("Failed to process internal message. Error: {e:#}"),
                }

                tokio::time::sleep(PROCESS_INCOMING_MESSAGES_INTERVAL).await;
            }
        }
    });

    let pending_matches = Arc::new(Mutex::new(Vec::<trade::MatchParams>::new()));
    tokio::spawn({
        let node = node.clone();
        let pending_matches = pending_matches.clone();
        let pending_confirmations = pending_confirmations.clone();
        async move {
            loop {
                {
                    let mut pending_matches = match pending_matches.lock() {
                        Ok(pending_matches) => pending_matches,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to acquire lock on pending matches. Error: {e:#}"
                            );
                            continue;
                        }
                    };

                    let mut pending_confirmations = match pending_confirmations.lock() {
                        Ok(pending_confirmations) => pending_confirmations,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to acquire lock on pending confirmations. Error {e:#}"
                            );
                            continue;
                        }
                    };

                    let mut confirmed_matches_indices = vec![];

                    for (index, pending_match) in pending_matches.iter().enumerate() {
                        if pending_confirmations
                            .get(&pending_match.maker.pub_key)
                            .is_some()
                            && pending_confirmations
                                .get(&pending_match.taker.pub_key)
                                .is_some()
                        {
                            tracing::info!("Both traders accepted the dlc proposal. Going to confirm the trade.");
                            let maker_confirmation = pending_confirmations
                                .remove(&pending_match.maker.pub_key)
                                .expect("confirmation message to exist");
                            node.send_sub_channel_message(
                                pending_match.maker.pub_key,
                                &maker_confirmation,
                            );

                            let taker_confirmation = pending_confirmations
                                .remove(&pending_match.taker.pub_key)
                                .expect("confirmation message to exist");
                            node.send_sub_channel_message(
                                pending_match.taker.pub_key,
                                &taker_confirmation,
                            );

                            confirmed_matches_indices.push(index);
                        }
                    }

                    tracing::debug!("Removing confirmed matches from pending matches");
                    for index in confirmed_matches_indices.iter().rev() {
                        pending_matches.remove(*index);
                    }
                }

                tokio::time::sleep(PROCESS_INCOMING_MESSAGES_INTERVAL).await;
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
            pending_matches,
            pending_confirmations,
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
