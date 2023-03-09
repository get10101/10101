use crate::api::Balances;
use crate::api::WalletInfo;
use crate::config;
use crate::event;
use crate::event::EventInternal;
use crate::trade::order;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::XOnlyPublicKey;
use lightning_invoice::Invoice;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::seed::Bip39Seed;
use ln_dlc_node::Dlc;
use state::Storage;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

static NODE: Storage<Arc<Node>> = Storage::new();

const PROCESS_TRADE_REQUESTS_INTERVAL: Duration = Duration::from_secs(30);

pub fn get_wallet_info() -> Result<WalletInfo> {
    Ok(get_wallet_info_from_node(
        NODE.try_get().context("failed to get ln dlc node")?,
    ))
}

fn get_wallet_info_from_node(node: &Node) -> WalletInfo {
    WalletInfo {
        balances: Balances {
            lightning: node.get_ldk_balance().available,
            on_chain: node.get_on_chain_balance().expect("balance").confirmed,
        },
        history: vec![], // TODO: sync history
    }
}

pub fn get_node_info() -> Result<NodeInfo> {
    Ok(NODE.try_get().context("failed to get ln dlc node")?.info)
}

// TODO: should we also wrap the oracle as `NodeInfo`. It would fit the required attributes pubkey
// and address.
pub fn get_oracle_pubkey() -> Result<XOnlyPublicKey> {
    Ok(NODE
        .try_get()
        .context("failed to get ln dlc node")?
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
            Node::new_app(
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

        // todo: should the library really be responsible for managing the task?
        node.keep_connected(config::get_coordinator_info()).await?;

        let node_clone = node.clone();
        runtime.spawn(async move {
            loop {
                // todo: the node sync should not swallow the error.
                node_clone.sync();
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                let wallet_info = get_wallet_info_from_node(&node_clone);
                event::publish(&EventInternal::WalletInfoUpdateNotification(wallet_info));
            }
        });

        let node_cloned = node.clone();
        // periodically update for positions
        runtime.spawn(async move {
            loop {
                let contracts = match node_cloned.get_confirmed_dlcs() {
                    Ok(contracts) => contracts,
                    Err(e) => {
                        tracing::error!("Failed to retrieve DLCs from node: {e:#}");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                // Assumes that there is only one contract, i.e. one position
                if let Some(Dlc {
                    offer_collateral, ..
                }) = contracts.get(0)
                {
                    if position::handler::is_position_up_to_date(offer_collateral) {
                        continue;
                    }

                    // TODO: I don't think we can get rid of this error scenarios, but they sucks
                  let filled_order = match order::handler::order_filled() {
                      Ok(filled_order) => filled_order,
                      Err(e) => {
                          tracing::error!("Critical Error! We have a DLC but were unable to set the order to filled: {e:#}");
                          continue;
                      }
                  };

                    if let Err(e) = position::handler::order_filled(filled_order, *offer_collateral) {
                        tracing::error!("Failed to handle position after receiving DLC: {e:#}");
                        continue;
                    }
                }

                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });

        tokio::spawn({
            let node = node.clone();
            async move {
                loop {
                    tokio::time::sleep(PROCESS_TRADE_REQUESTS_INTERVAL).await;

                    let coordinator_pubkey = config::get_coordinator_info().pubkey;
                    tracing::debug!(%coordinator_pubkey, "Checking for DLC offers");

                    let sub_channel = match node.get_sub_channel_offer(&coordinator_pubkey) {
                        Ok(Some(sub_channel)) => sub_channel,
                        Ok(None) => {
                            tracing::debug!(%coordinator_pubkey, "No DLC channel offers found");
                            continue;
                        },
                        Err(e) => {
                            tracing::error!(peer = %coordinator_pubkey.to_string(), "Unable to retrieve DLC channel offer: {e:#}");
                            continue;
                        }
                    };

                    tracing::info!(%coordinator_pubkey, "Found DLC channel offer");

                    let channel_id = sub_channel.channel_id;

                    // todo: the app should validate if the offered dlc channel matches it's submitted order.

                    tracing::info!(%coordinator_pubkey, channel_id = %hex::encode(channel_id), "Accepting DLC channel offer");

                    if let Err(e) = node.accept_dlc_channel_offer(&channel_id) {
                        tracing::error!(channel_id = %hex::encode(channel_id), "Failed to accept subchannel: {e:#}");
                    };
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
        .get_new_address()
        .map_err(|e| anyhow!("Failed to get new address: {e}"))?;
    Ok(address.to_string())
}

/// TODO: remove this function once the lightning faucet is more stable. This is only added for
/// testing purposes - so that we can quickly get funds into the lightning wallet.
pub fn open_channel() -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;

    node.initiate_open_channel(config::get_coordinator_info(), 500000, 250000)?;

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
    node.send_payment(&invoice)
}
