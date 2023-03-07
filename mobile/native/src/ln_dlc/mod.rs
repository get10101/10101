use crate::api::Balances;
use crate::api::WalletInfo;
use crate::common::api::Direction;
use crate::config;
use crate::event;
use crate::event::EventInternal;
use crate::trade::position;
use crate::trade::position::PositionStateTrade;
use crate::trade::position::PositionTrade;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::Network;
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
use trade::ContractSymbol;
use trade::TradeParams;

static NODE: Storage<Arc<Node>> = Storage::new();

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
    let network = Network::Regtest;
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
                    // TODO: Load position data from database and fill in the values; the collateral
                    // can be taken from the DLC
                    event::publish(&EventInternal::PositionUpdateNotification(PositionTrade {
                        leverage: 0.0,
                        quantity: 0.0,
                        contract_symbol: ContractSymbol::BtcUsd,
                        direction: Direction::Long,
                        average_entry_price: 0.0,
                        liquidation_price: 0.0,
                        unrealized_pnl: 0,
                        position_state: PositionStateTrade::Open,
                        collateral: *offer_collateral,
                    }));
                }

                tokio::time::sleep(Duration::from_secs(10)).await;
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

pub async fn trade(trade_params: TradeParams) -> Result<()> {
    let client = reqwest::Client::new();
    let contract_info = client
        .post(format!("http://{}/api/trade", config::get_http_endpoint()))
        .json(&trade_params)
        .send()
        .await
        .context("Failed to submit trade request to coordinator")?
        .json()
        .await?;

    let node = NODE.try_get().context("failed to get ln dlc node")?;

    let channel_details = node.list_usable_channels();
    let channel_details = channel_details
        .iter()
        .find(|c| c.counterparty.node_id == config::get_coordinator_info().pubkey)
        .context("Channel details not found")?;

    node.propose_dlc_channel(channel_details, &contract_info)
        .await
        .context("Failed to propose DLC channel with coordinator")?;
    tracing::info!("Proposed dlc subchannel");
    Ok(())
}
