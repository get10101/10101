use anyhow::Context;
use anyhow::Result;
use bitcoin::Network;
use coordinator::cli::Opts;
use coordinator::logger;
use ln_dlc_node::node::Node;
use ln_dlc_node::seed::Bip39Seed;
use rand::thread_rng;
use rand::RngCore;
use tracing::metadata::LevelFilter;

const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::read();
    let data_dir = opts.data_dir()?;
    let address = opts.p2p_address;
    let network = Network::Regtest;

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

    let node = Node::new(
        "coordinator".to_string(),
        network,
        data_dir.as_path(),
        address,
        ELECTRS_ORIGIN.to_string(),
        seed,
        ephemeral_randomness,
    )
    .await;

    let background_processor = node.start().await?;

    background_processor.join()?;

    Ok(())
}
