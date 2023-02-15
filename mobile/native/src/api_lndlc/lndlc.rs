use crate::api::Event;
use crate::api_lndlc::runtime;
use crate::api_lndlc::Balance;
use crate::api_lndlc::ELECTRS_ORIGIN;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::Network;
use dlc_manager::Wallet;
use flutter_rust_bridge::StreamSink;
use lightning_invoice::Invoice;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::seed::Bip39Seed;
use state::Storage;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

static NODE: Storage<Arc<Node>> = Storage::new();

const REGTEST_COORDINATOR_PK: &str =
    "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9";

// TODO: this configuration should not be hardcoded.
const HOST: &str = "127.0.0.1";
const HTTP_PORT: u16 = 8000;
const P2P_PORT: u16 = 9045;

pub fn get_coordinator_info() -> NodeInfo {
    NodeInfo {
        pubkey: REGTEST_COORDINATOR_PK
            .parse()
            .expect("Hard-coded PK to be valid"),
        address: format!("{HOST}:{P2P_PORT}") // todo: make ip configurable
            .parse()
            .expect("Hard-coded IP and port to be valid"),
    }
}

pub fn run(stream: StreamSink<Event>, data_dir: String) -> Result<()> {
    let network = Network::Regtest;
    let runtime = runtime()?;
    runtime.block_on(async move {
        stream.add(Event::Init("Starting full ldk node".to_string()));
        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let data_dir = Path::new(&data_dir).join(network.to_string());
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let seed_path = data_dir.join("seed");
        let seed = Bip39Seed::initialize(&seed_path)?;

        let node = Arc::new(
            Node::new_app(
                "10101".to_string(),
                network,
                data_dir.as_path(),
                address,
                ELECTRS_ORIGIN.to_string(),
                seed,
                ephemeral_randomness,
            )
            .await,
        );

        let background_processor = node.start().await?;

        // todo: should the library really be responsible for managing the task?
        node.keep_connected(get_coordinator_info()).await?;

        let node_clone = node.clone();
        runtime.spawn(async move {
            loop {
                // todo: the node sync should not swallow the error.
                node_clone.sync();
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                stream.add(Event::WalletInfo(Balance {
                    off_chain: node_clone.get_ldk_balance().unwrap().available,
                    on_chain: node_clone
                        .get_on_chain_balance()
                        .expect("balance")
                        .confirmed,
                }));
            }
        });

        runtime.spawn_blocking(move || {
            // background processor joins on a sync thread, meaning that join here will block a
            // full thread, which is dis-encouraged to do in async code.
            if let Err(err) = background_processor.join() {
                tracing::error!(?err, "Background processor stopped unexpected");
            }
        });

        NODE.set(node);

        Ok(())
    })
}

pub fn get_new_address() -> Result<String> {
    let node = NODE.try_get().unwrap();
    let address = node
        .wallet
        .get_new_address()
        .map_err(|e| anyhow!("Failed to get new address: {e}"))?;
    Ok(address.to_string())
}

pub fn open_channel() -> Result<()> {
    let node = NODE.try_get().unwrap();

    node.open_channel(get_coordinator_info(), 500000, 250000)
}

pub fn create_invoice() -> Result<Invoice> {
    let runtime = runtime()?;

    runtime.block_on(async {
        let node = NODE.try_get().unwrap();

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "http://{HOST}:{HTTP_PORT}/api/get_fake_scid/{}",
                node.info.pubkey
            )) // TODO: make host configurable
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            bail!("Failed to fetch fake scid from coordinator: {text}")
        }

        let text = response.text().await?;
        tracing::info!("Fetch fake channel id: {}", text);

        let fake_channel_id: u64 = text.parse()?;

        node.create_interceptable_invoice(
            1000,
            fake_channel_id,
            get_coordinator_info().pubkey,
            0,
            "test".to_string(),
        )
    })
}

pub fn send_payment(invoice: &str) -> Result<()> {
    let node = NODE.try_get().unwrap();
    let invoice = Invoice::from_str(invoice).context("Could not parse Invoice string")?;
    node.send_payment(&invoice)
}
