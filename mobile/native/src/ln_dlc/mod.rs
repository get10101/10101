mod accept;
mod node;

use crate::api::WalletInfo;
use crate::config;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::node::Node;
use crate::trade::order::FailureReason;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::XOnlyPublicKey;
use coordinator_commons::TradeParams;
use lightning_invoice::Invoice;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::seed::Bip39Seed;
use state::Storage;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

static NODE: Storage<Arc<Node>> = Storage::new();
const PROCESS_INCOMING_MESSAGES_INTERVAL: Duration = Duration::from_secs(5);

pub fn get_wallet_info() -> Result<WalletInfo> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let wallet_info = node.get_wallet_info_from_node();
    Ok(wallet_info)
}

pub fn get_node_key() -> Result<SecretKey> {
    NODE.try_get()
        .context("failed to get ln dlc node")?
        .inner
        .node_key()
}

pub fn get_node_info() -> Result<NodeInfo> {
    Ok(NODE
        .try_get()
        .context("failed to get ln dlc node")?
        .inner
        .info)
}

// TODO: should we also wrap the oracle as `NodeInfo`. It would fit the required attributes pubkey
// and address.
pub fn get_oracle_pubkey() -> Result<XOnlyPublicKey> {
    Ok(NODE
        .try_get()
        .context("failed to get ln dlc node")?
        .inner
        .oracle_pk())
}

/// Lazily creates a multi threaded runtime with the the number of worker threads corresponding to
/// the number of available cores.
fn runtime() -> Result<&'static Runtime> {
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}

pub fn run(data_dir: String) -> Result<()> {
    let network = config::get_network();
    let runtime = runtime()?;

    runtime.block_on(async move {
        event::publish(&EventInternal::Init("Starting full ldk node".to_string()));

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let data_dir = Path::new(&data_dir).join(network.to_string());
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        event::subscribe(position::subscriber::Subscriber {});
        // TODO: Subscribe to events from the orderbook and publish OrderFilledWith event

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0")?;
            listener.local_addr().expect("To get a free local address")
        };

        let seed_path = data_dir.join("seed");
        let seed = Bip39Seed::initialize(&seed_path)?;

        let node = Arc::new(
            ln_dlc_node::node::Node::new_app(
                "10101",
                network,
                data_dir.as_path(),
                address,
                config::get_electrs_endpoint().to_string(),
                seed,
                ephemeral_randomness,
            )
            .await?,
        );
        let node = Arc::new(Node { inner: node });

        // todo: should the library really be responsible for managing the task?
        node.inner
            .keep_connected(config::get_coordinator_info())
            .await?;

        // automatically accepts dlc channel offers (open and close)
        node.start_accept_offers_task()?;

        tokio::spawn({
            let node = node.clone();
            async move {
                loop {
                    if let Err(e) = node.process_incoming_messages() {
                        tracing::error!("Unable to process incoming messages: {e:#}");
                    }

                    tokio::time::sleep(PROCESS_INCOMING_MESSAGES_INTERVAL).await;
                }
            }
        });

        runtime.spawn({
            let node = node.clone();
            async move {
                loop {
                    // todo: the node sync should not swallow the error.
                    node.inner.sync();
                    tokio::time::sleep(Duration::from_secs(10)).await;

                    let wallet_info = node.get_wallet_info_from_node();
                    event::publish(&EventInternal::WalletInfoUpdateNotification(wallet_info));
                }
            }
        });

        NODE.set(node);

        Ok(())
    })
}

pub fn get_new_address() -> Result<String> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let address = node
        .inner
        .get_new_address()
        .map_err(|e| anyhow!("Failed to get new address: {e}"))?;
    Ok(address.to_string())
}

/// TODO: remove this function once the lightning faucet is more stable. This is only added for
/// testing purposes - so that we can quickly get funds into the lightning wallet.
pub fn open_channel() -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;

    node.inner
        .initiate_open_channel(config::get_coordinator_info(), 500000, 250000)?;

    Ok(())
}

pub fn create_invoice(amount_sats: Option<u64>) -> Result<Invoice> {
    let runtime = runtime()?;

    runtime.block_on(async {
        let node = NODE.try_get().context("failed to get ln dlc node")?;
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "http://{}/api/fake_scid/{}",
                config::get_http_endpoint(),
                node.inner.info.pubkey
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

        node.inner.create_interceptable_invoice(
            amount_sats,
            fake_channel_id,
            config::get_coordinator_info().pubkey,
            0,
            "test".to_string(),
        )
    })
}

pub fn send_payment(invoice: &str) -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let invoice = Invoice::from_str(invoice).context("Could not parse Invoice string")?;
    node.inner.send_payment(&invoice)
}

pub async fn trade(trade_params: TradeParams) -> Result<(), (FailureReason, anyhow::Error)> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/api/trade", config::get_http_endpoint()))
        .json(&trade_params)
        .send()
        .await
        .context("Failed to request trade with coordinator")
        .map_err(|e| (FailureReason::TradeRequest, e))?;

    if !response.status().is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };
        return Err((
            FailureReason::TradeResponse,
            anyhow!("Could not post trade to coordinator: {response_text}"),
        ));
    }

    tracing::info!("Sent trade request to coordinator successfully");

    Ok(())
}
