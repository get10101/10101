use crate::api::Balances;
use crate::api::WalletInfo;
use crate::event;
use crate::event::EventInternal;
use crate::trade::position;
use crate::trade::position::PositionStateTrade;
use crate::trade::position::PositionTrade;
use crate::trade::position::TradeParams;
use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::Network;
use dlc_manager::contract::Contract;
use dlc_manager::Wallet;
use lightning_invoice::Invoice;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::seed::Bip39Seed;
use state::Storage;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";

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

pub fn get_node_pubkey() -> bdk::bitcoin::secp256k1::PublicKey {
    NODE.try_get().unwrap().info.pubkey
}

// TODO: this model should not be in the event!
#[derive(Debug, Eq, Hash, PartialEq, Clone, Default)]
pub struct Balance {
    pub on_chain: u64,
    pub off_chain: u64,
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
        // runtime.spawn(async move {
        // loop {
        // TODO: Subscribe to events from the orderbook and publish OrderFilledWith event
        // event::publish(&EventInternal::OrderFilledWith(TradeParams {
        //     taker_node_pubkey: bdk::bitcoin::secp256k1::PublicKey::from_str(
        //         "02e6642fd69bd211f93f7f1f36ca51a26a5290eb2dd1b0d8279a87bb0d480c8443",
        //     )
        //     .unwrap(),
        //     contract_input: ContractInput {},
        // }));
        // }
        // });

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

                event::publish(&EventInternal::WalletInfoUpdateNotification(WalletInfo {
                    balances: Balances {
                        lightning: node_clone.get_ldk_balance().available,
                        on_chain: node_clone
                            .get_on_chain_balance()
                            .expect("balance")
                            .confirmed,
                    },
                    history: vec![], // TODO: sync history
                }));
            }
        });

        let node_cloned = node.clone();
        // periodically update for positions
        runtime.spawn(async move {
            loop {
                let contracts = node_cloned.get_contracts().unwrap();

                // Assumes that there is only one contract, i.e. one position
                if let Some(Contract::Confirmed(contract)) = contracts.get(0) {
                    // TODO: Load position data from database and fill in the values; the collateral
                    // can be taken from the DLC
                    event::publish(&EventInternal::PositionUpdateNotification(PositionTrade {
                        leverage: 0.0,
                        quantity: 0.0,
                        contract_symbol: ContractSymbolTrade::BtcUsd,
                        direction: DirectionTrade::Long,
                        average_entry_price: 0.0,
                        liquidation_price: 0.0,
                        unrealized_pnl: 0,
                        position_state: PositionStateTrade::Open,
                        collateral: contract.accepted_contract.accept_params.collateral,
                    }));
                }

                tokio::time::sleep(Duration::from_secs(10)).await;
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
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let address = node
        .wallet
        .get_new_address()
        .map_err(|e| anyhow!("Failed to get new address: {e}"))?;
    Ok(address.to_string())
}

/// TODO: remove this function once the lightning faucet is more stable. This is only added for
/// testing purposes - so that we can quickly get funds into the lightning wallet.
pub fn open_channel() -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;

    node.initiate_open_channel(get_coordinator_info(), 500000, 250000)?;

    Ok(())
}

pub fn create_invoice() -> Result<Invoice> {
    let runtime = runtime()?;

    runtime.block_on(async {
        let node = NODE.try_get().context("failed to get ln dlc node")?;

        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "http://{HOST}:{HTTP_PORT}/api/fake_scid/{}",
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
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let invoice = Invoice::from_str(invoice).context("Could not parse Invoice string")?;
    node.send_payment(&invoice)
}

pub async fn trade(trade_params: TradeParams) -> Result<()> {
    let url = "http://localhost:8000/api/trade"; // TODO: we need the coordinators http address here

    let client = reqwest::Client::new();
    client
        .post(url)
        .json(&trade_params)
        .send()
        .await
        .context("Failed to submit trade request to coordinator")?;

    let node = NODE.try_get().context("failed to get ln dlc node")?;

    let channel_details = node.list_usable_channels();
    let channel_details = channel_details
        .iter()
        .find(|c| c.counterparty.node_id == get_coordinator_info().pubkey)
        .unwrap();

    node.propose_dlc_channel(channel_details, &trade_params.contract_input.into())
        .await
        .context("Failed to propose DLC channel with coordinator")?;
    tracing::info!("Proposed dlc subchannel");
    Ok(())
}
