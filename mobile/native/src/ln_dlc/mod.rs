use crate::api;
use crate::api::Fee;
use crate::api::PaymentFlow;
use crate::api::SendPayment;
use crate::api::Status;
use crate::api::WalletHistoryItem;
use crate::api::WalletHistoryItemType;
use crate::backup::DBBackupSubscriber;
use crate::commons::reqwest_client;
use crate::config;
use crate::config::get_rgs_server_url;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::channel_status::track_channel_status;
use crate::ln_dlc::node::Node;
use crate::ln_dlc::node::NodeStorage;
use crate::ln_dlc::node::WalletHistories;
use crate::state;
use crate::trade::order;
use crate::trade::order::FailureReason;
use crate::trade::order::Order;
use crate::trade::order::OrderReason;
use crate::trade::order::OrderState;
use crate::trade::order::OrderType;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::Txid;
use bdk::bitcoin::XOnlyPublicKey;
use bdk::Balance;
use bdk::BlockTime;
use bdk::FeeRate;
use bdk::TransactionDetails;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::SECP256K1;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::PackedLockTime;
use bitcoin::Transaction;
use bitcoin::TxIn;
use bitcoin::TxOut;
use commons::CollaborativeRevertTraderResponse;
use commons::OnboardingParam;
use commons::RouteHintHop;
use commons::TradeParams;
use itertools::chain;
use itertools::Itertools;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::events::Event;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::ln::ChannelId;
use lightning::sign::KeysManager;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::channel::UserChannelId;
use ln_dlc_node::config::app_config;
use ln_dlc_node::lightning_invoice::Bolt11Invoice;
use ln_dlc_node::node::rust_dlc_manager;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LnDlcChannelSigner;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LnDlcSignerProvider;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannel;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannelState;
use ln_dlc_node::node::rust_dlc_manager::Storage as DlcStorage;
use ln_dlc_node::node::GossipSourceConfig;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::node::Storage as LnDlcNodeStorage;
use ln_dlc_node::scorer;
use ln_dlc_node::seed::Bip39Seed;
use ln_dlc_node::util;
use ln_dlc_node::AppEventHandler;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::WalletSettings;
use ln_dlc_node::CONFIRMATION_TARGET;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio::task::spawn_blocking;
use trade::ContractSymbol;

mod lightning_subscriber;
pub mod node;
mod recover_rollover;
mod sync_position_to_subchannel;

pub mod channel_status;

use crate::storage::TenTenOneNodeStorage;
pub use channel_status::ChannelStatus;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannel;

const PROCESS_INCOMING_DLC_MESSAGES_INTERVAL: Duration = Duration::from_millis(200);
const UPDATE_WALLET_HISTORY_INTERVAL: Duration = Duration::from_secs(5);
const CHECK_OPEN_ORDERS_INTERVAL: Duration = Duration::from_secs(60);
const ON_CHAIN_SYNC_INTERVAL: Duration = Duration::from_secs(300);
const WAIT_FOR_CONNECTING_TO_COORDINATOR: Duration = Duration::from_secs(2);

/// Defines a constant from which we treat a transaction as confirmed
const NUMBER_OF_CONFIRMATION_FOR_BEING_CONFIRMED: u64 = 1;

/// The weight estimate of the funding transaction
///
/// This weight estimate assumes two inputs.
/// This value was chosen based on mainnet channel funding transactions with two inputs.
/// Note that we cannot predict this value precisely, because the app cannot predict what UTXOs the
/// coordinator will use for the channel opening transaction. Only once the transaction is know the
/// exact fee will be know.
pub const FUNDING_TX_WEIGHT_ESTIMATE: u64 = 220;

/// Triggers an update to the wallet balance and history, without an on-chain sync.
pub fn refresh_lightning_wallet() -> Result<()> {
    let node = state::get_node();
    if let Err(e) = node.inner.sync_lightning_wallet() {
        tracing::error!("Manually triggered Lightning wallet sync failed: {e:#}");
    }

    if let Err(e) = keep_wallet_balance_and_history_up_to_date(&node) {
        tracing::error!("Failed to keep wallet history up to date: {e:#}");
    }

    Ok(())
}

/// Trigger an on-chain sync followed by an update to the wallet balance and history.
///
/// We do not wait for the triggered task to finish, because the effect will be reflected
/// asynchronously on the UI.
pub async fn refresh_wallet_info() -> Result<()> {
    let node = state::get_node();
    let wallet = node.inner.ldk_wallet();

    // Spawn into the blocking thread pool of the dedicated backend runtime to avoid blocking the UI
    // thread.
    let runtime = state::get_or_create_tokio_runtime()?;
    runtime.spawn_blocking(move || {
        if let Err(e) = wallet.sync() {
            tracing::error!("Manually triggered on-chain sync failed: {e:#}");
        }

        if let Err(e) = node.inner.sync_lightning_wallet() {
            tracing::error!("Manually triggered Lightning wallet sync failed: {e:#}");
        }

        if let Err(e) = keep_wallet_balance_and_history_up_to_date(&node) {
            tracing::error!("Failed to keep wallet history up to date: {e:#}");
        }

        anyhow::Ok(())
    });

    Ok(())
}

pub fn get_seed_phrase() -> Vec<String> {
    state::get_seed().get_seed_phrase()
}

/// Gets the seed from the storage or from disk. However it will panic if the seed can not be found.
/// No new seed will be created.
fn get_seed() -> Bip39Seed {
    match state::try_get_seed() {
        Some(seed) => seed,
        None => {
            let seed_dir = config::get_seed_dir();

            let network = config::get_network();
            let seed_path = Path::new(&seed_dir).join(network.to_string()).join("seed");
            assert!(seed_path.exists());

            let seed = Bip39Seed::initialize(&seed_path).expect("to read seed file");
            state::set_seed(seed.clone());
            seed
        }
    }
}

pub fn get_node_key() -> SecretKey {
    match state::try_get_node() {
        Some(node) => node.inner.node_key(),
        None => {
            let seed = get_seed();
            let time_since_unix_epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("unix epos to not be earlier than now");
            let keys_manger = KeysManager::new(
                &seed.lightning_seed(),
                time_since_unix_epoch.as_secs(),
                time_since_unix_epoch.subsec_nanos(),
            );
            keys_manger.get_node_secret_key()
        }
    }
}

pub fn get_node_pubkey() -> PublicKey {
    get_node_key().public_key(SECP256K1)
}

pub async fn update_node_settings(settings: LnDlcNodeSettings) {
    let node = state::get_node();
    node.inner.update_settings(settings).await;
}

pub fn get_oracle_pubkey() -> XOnlyPublicKey {
    state::get_node().inner.oracle_pubkey
}

pub fn get_funding_transaction(channel_id: &ChannelId) -> Result<Txid> {
    let node = state::get_node();
    let channel_details = node.inner.channel_manager.get_channel_details(channel_id);

    let funding_transaction = match channel_details {
        Some(channel_details) => match channel_details.funding_txo {
            Some(funding_txo) => funding_txo.txid,
            None => bail!(
                "Could not find funding transaction for channel {}",
                hex::encode(channel_id.0)
            ),
        },
        None => bail!(
            "Could not find channel details for {}",
            hex::encode(channel_id.0)
        ),
    };

    Ok(funding_transaction)
}

/// Gets the 10101 node storage, initializes the storage if not found yet.
pub fn get_storage() -> TenTenOneNodeStorage {
    match state::try_get_storage() {
        Some(storage) => storage,
        None => {
            // storage is only initialized before the node is started if a new wallet is created
            // or restored.
            let storage = TenTenOneNodeStorage::new(
                config::get_data_dir(),
                config::get_network(),
                get_node_key(),
            );
            tracing::info!("Initialized 10101 storage!");
            state::set_storage(storage.clone());
            storage
        }
    }
}

/// Start the node
///
/// Allows specifying a data directory and a seed directory to decouple
/// data and seed storage (e.g. data is useful for debugging, seed location
/// should be more protected).
pub fn run(seed_dir: String, runtime: &Runtime) -> Result<()> {
    let network = config::get_network();

    runtime.block_on(async move {
        event::publish(&EventInternal::Init("Starting full ldk node".to_string()));

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        // TODO: Subscribe to events from the orderbook and publish OrderFilledWith event

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0")?;
            listener.local_addr().expect("To get a free local address")
        };

        let seed_dir = Path::new(&seed_dir).join(network.to_string());
        let seed_path = seed_dir.join("seed");
        let seed = Bip39Seed::initialize(&seed_path)?;
        state::set_seed(seed.clone());

        let (event_sender, event_receiver) = watch::channel::<Option<Event>>(None);

        let node_storage = Arc::new(NodeStorage);

        let storage = get_storage();

        event::subscribe(DBBackupSubscriber::new(storage.clone().client));

        let node = ln_dlc_node::node::Node::new(
            app_config(),
            scorer::in_memory_scorer,
            "10101",
            network,
            Path::new(&storage.data_dir),
            storage.clone(),
            node_storage,
            address,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
            util::into_socket_addresses(address),
            config::get_esplora_endpoint(),
            seed,
            ephemeral_randomness,
            ln_dlc_node_settings(),
            WalletSettings::default(),
            vec![config::get_oracle_info().into()],
            config::get_oracle_info().public_key,
        )?;
        let node = Arc::new(node);

        let event_handler = AppEventHandler::new(node.clone(), Some(event_sender));
        let _running = node.start(event_handler, true)?;
        let node = Arc::new(Node::new(node, _running));

        // Refresh the wallet balance and history eagerly so that it can complete before the
        // triggering the first on-chain sync. This ensures that the UI appears ready as soon as
        // possible.
        //
        // TODO: This might not be necessary once we rewrite the on-chain wallet with bdk:1.0.0.
        spawn_blocking({
            let node = node.clone();
            move || keep_wallet_balance_and_history_up_to_date(&node)
        })
        .await
        .expect("task to complete")?;

        runtime.spawn({
            let node = node.clone();
            async move {
                loop {
                    tokio::time::sleep(UPDATE_WALLET_HISTORY_INTERVAL).await;

                    let node = node.clone();
                    if let Err(e) =
                        spawn_blocking(move || keep_wallet_balance_and_history_up_to_date(&node))
                            .await
                            .expect("To spawn blocking task")
                    {
                        tracing::error!("Failed to sync balance and wallet history: {e:#}");
                    }
                }
            }
        });

        std::thread::spawn({
            let node = node.clone();
            move || loop {
                if let Err(e) = node.inner.sync_on_chain_wallet() {
                    tracing::error!("Failed on-chain sync: {e:#}");
                }

                std::thread::sleep(ON_CHAIN_SYNC_INTERVAL);
            }
        });

        runtime.spawn({
            let node = node.clone();
            async move { node.listen_for_lightning_events(event_receiver).await }
        });
        let coordinator_info = config::get_coordinator_info();
        let coordinator_pk = coordinator_info.pubkey;

        runtime.spawn({
            let node = node.clone();
            async move { node.keep_connected(coordinator_info).await }
        });

        runtime.spawn({
            let node = node.clone();
            async move {
                loop {
                    let node = node.clone();
                    spawn_blocking(move || node.process_incoming_dlc_messages())
                        .await
                        .expect("To spawn blocking thread");
                    tokio::time::sleep(PROCESS_INCOMING_DLC_MESSAGES_INTERVAL).await;
                }
            }
        });

        runtime.spawn(async move {
            loop {
                if let Err(e) = spawn_blocking(order::handler::check_open_orders)
                    .await
                    .expect("To spawn blocking task")
                {
                    tracing::error!("Error while checking open orders: {e:#}");
                }

                tokio::time::sleep(CHECK_OPEN_ORDERS_INTERVAL).await;
            }
        });

        runtime.spawn(track_channel_status(node.clone()));

        let inner_node = node.clone();

        runtime.spawn_blocking(move || {
            let node = inner_node.clone();
            async move {
                let mut iteration_count = 0;

                while !node
                    .inner
                    .peer_manager
                    .get_peer_node_ids()
                    .iter()
                    .any(|(a, _)| a == &coordinator_pk)
                {
                    tracing::trace!(
                        "Not yet connecting to coordinator. Waiting with recovery for a connection"
                    );

                    tokio::time::sleep(WAIT_FOR_CONNECTING_TO_COORDINATOR).await;

                    iteration_count += 1;

                    if iteration_count >= 30 {
                        // After 30 retries (randonly chosen) we give up and continue with the
                        // function nevertheless. Which means, we might see
                        // an error.
                        break;
                    }
                }
                if let Err(e) = node.sync_position_with_subchannel_state().await {
                    tracing::error!("Failed to sync position with subchannel state. Error: {e:#}");
                }

                if let Err(e) = node.recover_rollover().await {
                    tracing::error!(
                        "Failed to check and recover from a stuck rollover state. Error: {e:#}"
                    );
                }
            }
        });

        state::set_node(node);

        event::publish(&EventInternal::Init("10101 is ready.".to_string()));

        Ok(())
    })
}

pub fn init_new_mnemonic(target_seed_file: &Path) -> Result<()> {
    let seed = Bip39Seed::initialize(target_seed_file)?;
    state::set_seed(seed);
    Ok(())
}

pub async fn restore_from_mnemonic(seed_words: &str, target_seed_file: &Path) -> Result<()> {
    let seed = Bip39Seed::restore_from_mnemonic(seed_words, target_seed_file)?;
    state::set_seed(seed);

    let storage = TenTenOneNodeStorage::new(
        config::get_data_dir(),
        config::get_network(),
        get_node_key(),
    );
    tracing::info!("Initialized 10101 storage!");
    state::set_storage(storage.clone());
    storage.client.restore(storage.dlc_storage).await
}

fn keep_wallet_balance_and_history_up_to_date(node: &Node) -> Result<()> {
    let wallet_balances = node
        .get_wallet_balances()
        .context("Failed to get wallet balances")?;

    let WalletHistories {
        on_chain,
        off_chain,
    } = node
        .get_wallet_histories()
        .context("Failed to get wallet histories")?;

    let blockchain_height = node.get_blockchain_height()?;

    // Get all channel related transactions (channel opening/closing). If we have seen a transaction
    // in the wallet it means the transaction has been published. If so, we remove it from
    // [`on_chain`] and add it as it's own WalletHistoryItem so that it can be displayed nicely.
    let dlc_channels = node.inner.list_dlc_channels()?;

    let dlc_channel_funding_tx_details = on_chain.iter().filter_map(|details| {
        match dlc_channels
            .iter()
            .find(|item| item.fund_tx.txid() == details.txid)
        {
            None => None,
            Some(channel) => {
                let (timestamp, n_confirmations) =
                    extract_timestamp_and_blockheight(blockchain_height, details);

                let status = if n_confirmations >= NUMBER_OF_CONFIRMATION_FOR_BEING_CONFIRMED {
                    Status::Confirmed
                } else {
                    Status::Pending
                };

                Some(WalletHistoryItem {
                    flow: PaymentFlow::Outbound,
                    amount_sats: details.sent - details.received,
                    timestamp,
                    status,
                    wallet_type: WalletHistoryItemType::DlcChannelFunding {
                        funding_txid: details.txid.to_string(),
                        // this is not 100% correct as fees are not exactly divided by 2. The fee a
                        // user has to pay depends on his final address.
                        reserved_fee_sats: details.fee.map(|fee| fee / 2),
                        confirmations: n_confirmations,
                        our_channel_input_amount_sats: channel.own_params.collateral,
                    },
                })
            }
        }
    });

    let on_chain = on_chain.iter().filter(|tx| {
        !dlc_channels
            .iter()
            .any(|channel| channel.fund_tx.txid() == tx.txid)
    });

    let on_chain = on_chain.map(|details| {
        let net_sats = details.received as i64 - details.sent as i64;

        let (flow, amount_sats) = if net_sats >= 0 {
            (PaymentFlow::Inbound, net_sats as u64)
        } else {
            (PaymentFlow::Outbound, net_sats.unsigned_abs())
        };

        let (timestamp, n_confirmations) =
            extract_timestamp_and_blockheight(blockchain_height, details);

        let status = if n_confirmations >= NUMBER_OF_CONFIRMATION_FOR_BEING_CONFIRMED {
            Status::Confirmed
        } else {
            Status::Pending
        };

        let wallet_type = WalletHistoryItemType::OnChain {
            txid: details.txid.to_string(),
            fee_sats: details.fee,
            confirmations: n_confirmations,
        };

        WalletHistoryItem {
            flow,
            amount_sats,
            timestamp,
            status,
            wallet_type,
        }
    });

    let off_chain = off_chain.iter().filter_map(|details| {
        tracing::trace!(details = %details, "Off-chain payment details");

        let amount_sats = match details.amount_msat {
            Some(msat) => msat / 1_000,
            // Skip payments that don't yet have an amount associated
            None => return None,
        };

        let decoded_invoice = match details.invoice.as_deref().map(Bolt11Invoice::from_str) {
            Some(Ok(inv)) => {
                tracing::trace!(?inv, "Decoded invoice");
                Some(inv)
            }
            Some(Err(err)) => {
                tracing::warn!(%err, "Failed to deserialize invoice");
                None
            }
            None => None,
        };

        let expired = decoded_invoice
            .as_ref()
            .map(|inv| inv.is_expired())
            .unwrap_or(false);

        let status = match details.status {
            HTLCStatus::Pending if expired => Status::Expired,
            HTLCStatus::Pending => Status::Pending,
            HTLCStatus::Succeeded => Status::Confirmed,
            HTLCStatus::Failed => Status::Failed,
        };

        let flow = match details.flow {
            ln_dlc_node::PaymentFlow::Inbound => PaymentFlow::Inbound,
            ln_dlc_node::PaymentFlow::Outbound => PaymentFlow::Outbound,
        };

        let timestamp = details.timestamp.unix_timestamp() as u64;

        let payment_hash = hex::encode(details.payment_hash.0);

        let expiry_timestamp = decoded_invoice
            .and_then(|inv| inv.timestamp().checked_add(inv.expiry_time()))
            .map(|time| OffsetDateTime::from(time).unix_timestamp() as u64);

        let wallet_type = WalletHistoryItemType::Lightning {
            payment_hash,
            description: details.description.clone(),
            payment_preimage: details.preimage.clone(),
            invoice: details.invoice.clone(),
            fee_msat: details.fee_msat,
            expiry_timestamp,
            funding_txid: details.funding_txid.clone(),
        };

        Some(WalletHistoryItem {
            flow,
            amount_sats,
            timestamp,
            status,
            wallet_type,
        })
    });

    let trades = db::get_all_trades()?;

    // We reverse the `Trade`s so that they are already pre-sorted _from oldest to newest_ in terms
    // of insertion. This is important because we sometimes insert `Trade`s back-to-back, so the
    // timestamps can coincide.
    let trades = trades.iter().rev().map(|trade| {
        let flow = if trade.trade_cost.is_positive() {
            PaymentFlow::Outbound
        } else {
            PaymentFlow::Inbound
        };

        let amount_sats = trade.trade_cost.abs().to_sat() as u64;

        let timestamp = trade.timestamp;

        // TODO: Add context about direction + contracts!
        WalletHistoryItem {
            flow,
            amount_sats,
            timestamp: timestamp.unix_timestamp() as u64,
            status: Status::Confirmed,
            wallet_type: WalletHistoryItemType::Trade {
                order_id: trade.order_id.to_string(),
                fee_sat: trade.fee.to_sat(),
                pnl: trade.pnl.map(|pnl| pnl.to_sat()),
                contracts: trade
                    .contracts
                    .ceil()
                    .to_u64()
                    .expect("Decimal to fit into u64"),
                direction: trade.direction.to_string(),
            },
        }
    });

    let history = chain![on_chain, off_chain, trades, dlc_channel_funding_tx_details]
        .sorted_by(|a, b| b.timestamp.cmp(&a.timestamp))
        .collect();

    let wallet_info = api::WalletInfo {
        balances: wallet_balances.into(),
        history,
    };

    event::publish(&EventInternal::WalletInfoUpdateNotification(wallet_info));

    Ok(())
}

fn extract_timestamp_and_blockheight(
    blockchain_height: u64,
    details: &TransactionDetails,
) -> (u64, u64) {
    match details.confirmation_time {
        Some(BlockTime { timestamp, height }) => (
            timestamp,
            // This is calculated manually to avoid wasteful requests to esplora,
            // since we can just cache the blockchain height as opposed to fetching it
            // for each block as with
            // `LnDlcWallet::get_transaction_confirmations`
            blockchain_height
                .checked_sub(height as u64)
                .unwrap_or_default(),
        ),

        None => {
            (
                // Unconfirmed transactions should appear towards the top of the
                // history
                OffsetDateTime::now_utc().unix_timestamp() as u64,
                0,
            )
        }
    }
}

pub fn get_unused_address() -> String {
    state::get_node().inner.get_unused_address().to_string()
}

pub async fn close_channel(is_force_close: bool) -> Result<()> {
    tracing::info!(force = is_force_close, "Offering to close a channel");
    let node = state::try_get_node().context("failed to get ln dlc node")?;

    let channels = node.inner.list_dlc_channels()?;
    let channel_details = channels.first().context("No channel to close")?;

    node.inner
        .close_dlc_channel(channel_details.channel_id, is_force_close)
        .await
}

pub fn get_dlc_channels() -> Result<Vec<SignedChannel>> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    node.inner.list_dlc_channels()
}

pub fn get_onchain_balance() -> Result<Balance> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    node.inner.get_on_chain_balance()
}

pub fn collaborative_revert_channel(
    channel_id: ChannelId,
    coordinator_address: Address,
    coordinator_amount: Amount,
    trader_amount: Amount,
    execution_price: Decimal,
    funding_txo: OutPoint,
) -> Result<()> {
    let node = state::try_get_node().context("Failed to get Node")?;
    let node = node.inner.clone();

    let channel_id_hex = hex::encode(channel_id.0);

    let subchannels = node.list_sub_channels()?;
    let subchannel = subchannels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .with_context(|| format!("Could not find subchannel {channel_id_hex}"))?;

    let channel_keys_id = subchannel
        .channel_keys_id
        .or(node
            .channel_manager
            .get_channel_details(&subchannel.channel_id)
            .map(|details| details.channel_keys_id))
        .with_context(|| {
            format!("Could not get channel keys ID for subchannel {channel_id_hex}")
        })?;

    let mut collab_revert_tx = Transaction {
        version: 2,
        lock_time: PackedLockTime::ZERO,
        input: vec![TxIn {
            previous_output: funding_txo,
            script_sig: Default::default(),
            sequence: Default::default(),
            witness: Default::default(),
        }],
        output: Vec::new(),
    };

    {
        let coordinator_script_pubkey = coordinator_address.script_pubkey();
        let dust_limit = coordinator_script_pubkey.dust_value();

        if coordinator_amount >= dust_limit {
            let txo = TxOut {
                value: coordinator_amount.to_sat(),
                script_pubkey: coordinator_script_pubkey,
            };

            collab_revert_tx.output.push(txo);
        } else {
            tracing::info!(
                %dust_limit,
                "Skipping coordinator output for collaborative revert transaction because \
                 it would be below the dust limit"
            )
        }
    };

    {
        let trader_script_pubkey = node.get_unused_address().script_pubkey();
        let dust_limit = trader_script_pubkey.dust_value();

        if trader_amount >= dust_limit {
            let txo = TxOut {
                value: trader_amount.to_sat(),
                script_pubkey: trader_script_pubkey,
            };

            collab_revert_tx.output.push(txo);
        } else {
            tracing::info!(
                %dust_limit,
                "Skipping trader output for collaborative revert transaction because \
                 it would be below the dust limit"
            )
        }
    };

    let own_sig = {
        let signer = node
            .keys_manager
            .derive_ln_dlc_channel_signer(subchannel.fund_value_satoshis, channel_keys_id);

        signer
            .get_holder_split_tx_signature(
                &Secp256k1::new(),
                &collab_revert_tx,
                &subchannel.original_funding_redeemscript,
                subchannel.fund_value_satoshis,
            )
            .context("Could not get own signature for collaborative revert transaction")?
    };

    let data = CollaborativeRevertTraderResponse {
        channel_id: channel_id_hex,
        transaction: collab_revert_tx,
        signature: own_sig,
    };

    let client = reqwest_client();
    let runtime = state::get_or_create_tokio_runtime()?;
    runtime.spawn({
        let subchannel = subchannel.clone();
        async move {
            match client
                .post(format!(
                    "http://{}/api/channels/revertconfirm",
                    config::get_http_endpoint(),
                ))
                .json(&data)
                .send()
                .await
            {
                Ok(response) => match response.text().await {
                    Ok(response) => {
                        tracing::info!(
                            response,
                            "Received response from confirming reverting a channel"
                        );
                        if let Err(e) =
                            update_state_after_collab_revert(subchannel, execution_price)
                        {
                            tracing::error!(
                                "Failed to update state after collaborative revert confirmation: {e:#}"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to decode collaborative revert confirmation response text: {e:#}"
                        );
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to confirm collaborative revert: {e:#}");
                }
            }
        }
    });

    Ok(())
}

fn update_state_after_collab_revert(
    sub_channel: SubChannel,
    execution_price: Decimal,
) -> Result<()> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    let positions = db::get_positions()?;

    let position = match positions.first() {
        Some(position) => {
            tracing::info!("Channel is reverted before the position got closed successfully.");
            position
        }
        None => {
            tracing::info!("Channel is reverted before the position got opened successfully.");
            if let Some(order) = db::get_order_in_filling()? {
                order::handler::order_failed(
                    Some(order.id),
                    FailureReason::CollabRevert,
                    anyhow!("Order failed due collab revert of the channel"),
                )?;
            }
            return Ok(());
        }
    };

    let filled_order = match order::handler::order_filled() {
        Ok(order) => order,
        Err(_) => {
            let order = Order {
                id: uuid::Uuid::new_v4(),
                leverage: position.leverage,
                quantity: position.quantity,
                contract_symbol: position.contract_symbol,
                direction: position.direction.opposite(),
                order_type: OrderType::Market,
                state: OrderState::Filled {
                    execution_price: execution_price.to_f32().expect("to fit into f32"),
                },
                creation_timestamp: OffsetDateTime::now_utc(),
                order_expiry_timestamp: OffsetDateTime::now_utc(),
                reason: OrderReason::Expired,
                stable: position.stable,
                failure_reason: None,
            };
            db::insert_order(order)?;
            event::publish(&EventInternal::OrderUpdateNotification(order));
            order
        }
    };

    position::handler::update_position_after_dlc_closure(Some(filled_order))?;

    match db::delete_positions() {
        Ok(_) => {
            event::publish(&EventInternal::PositionCloseNotification(
                ContractSymbol::BtcUsd,
            ));
        }
        Err(error) => {
            tracing::error!("Could not delete position : {error:#}");
        }
    }

    let mut sub_channel = sub_channel;
    sub_channel.state = SubChannelState::OnChainClosed;

    let node = node.inner.clone();

    node.dlc_manager
        .get_store()
        .upsert_sub_channel(&sub_channel)
        .map_err(|e| anyhow!("{e:#}"))
}

pub fn get_usable_channel_details() -> Result<Vec<ChannelDetails>> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    let channels = node.inner.list_usable_channels();

    Ok(channels)
}

pub fn get_fee_rate() -> Result<FeeRate> {
    get_fee_rate_for_target(CONFIRMATION_TARGET)
}

pub fn get_fee_rate_for_target(target: ConfirmationTarget) -> Result<FeeRate> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    Ok(node.inner.ldk_wallet().get_fee_rate(target))
}

/// Returns channel value or zero if there is no channel yet.
///
/// This is used when checking max tradeable amount
pub fn max_channel_value() -> Result<Amount> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    if let Some(existing_channel) = node
        .inner
        .list_channels()
        .first()
        .map(|c| c.channel_value_satoshis)
    {
        Ok(Amount::from_sat(existing_channel))
    } else {
        Ok(Amount::ZERO)
    }
}

pub fn contract_tx_fee_rate() -> Result<Option<u64>> {
    let node = state::try_get_node().context("failed to get ln dlc node")?;
    let fee_rate_per_vb = node
        .inner
        .list_sub_channels()?
        .first()
        .map(|c| c.fee_rate_per_vb);

    Ok(fee_rate_per_vb)
}

pub fn create_onboarding_invoice(
    liquidity_option_id: i32,
    amount_sats: u64,
    fee_sats: u64,
) -> Result<Bolt11Invoice> {
    let runtime = state::get_or_create_tokio_runtime()?;

    runtime.block_on(async {
        let node = state::get_node();
        let client = reqwest_client();

        // check if we have already announced a channel before. If so we can reuse the `user_channel_id`
        // the user navigates to the invoice screen.
        let channel = db::get_announced_channel(config::get_coordinator_info().pubkey)?;

        let user_channel_id = match channel {
            Some(channel) => channel.user_channel_id,
            None => {
                let user_channel_id = UserChannelId::new();
                let channel = Channel::new_jit_channel(
                    user_channel_id,
                    config::get_coordinator_info().pubkey,
                    liquidity_option_id,
                    fee_sats,
                );
                node.inner
                    .node_storage
                    .upsert_channel(channel)
                    .with_context(|| {
                        format!(
                            "Failed to insert shadow JIT channel with user channel id {user_channel_id}"
                        )
                    })?;

                user_channel_id
            },
        };

        tracing::info!(
            %user_channel_id,
        );

        let final_route_hint_hop : RouteHintHop = match client
            .post(format!(
                "http://{}/api/prepare_onboarding_payment",
                config::get_http_endpoint(),
            ))
            .json(&OnboardingParam {
                target_node: node.inner.info.pubkey.to_string(),
                user_channel_id: user_channel_id.to_string(),
                liquidity_option_id,
                amount_sats,
            })
            .send()
            .await?.error_for_status() {
                Ok(resp) => resp.json().await?,
                Err(e) => if e.status() == Some(reqwest::StatusCode::SERVICE_UNAVAILABLE) {
                    // Hack: Do not change the string below as it's matched in the frontend
                    bail!("Coordinator cannot provide required liquidity");
                } else {
                    bail!("Failed to fetch route hint from coordinator: {e:#}")
                }
            };

        node.inner.create_invoice_with_route_hint(
            Some(amount_sats),
            None,
            "Fund your 10101 wallet".to_string(),
            final_route_hint_hop.into(),
        )
    })
}

pub fn create_invoice(amount_sats: Option<u64>, description: String) -> Result<Bolt11Invoice> {
    let node = state::get_node();

    let final_route_hint_hop = node
        .inner
        .prepare_payment_with_route_hint(config::get_coordinator_info().pubkey)?;

    node.inner
        .create_invoice_with_route_hint(amount_sats, None, description, final_route_hint_hop)
}

pub fn create_usdp_invoice(amount_sats: Option<u64>, description: String) -> Result<Bolt11Invoice> {
    let invoice = create_invoice(amount_sats, description)?;

    let node = state::get_node();
    let mut write_guard = node.pending_usdp_invoices.lock();
    write_guard.insert(*invoice.payment_hash());

    Ok(invoice)
}

pub fn is_usdp_payment(payment_hash: String) -> bool {
    let node = state::get_node();
    let registered_usdp_invoice = node.pending_usdp_invoices.lock();

    registered_usdp_invoice
        .iter()
        .any(|hash| hash.to_string() == payment_hash)
}

pub async fn send_payment(payment: SendPayment) -> Result<()> {
    match payment {
        SendPayment::Lightning { invoice, amount } => {
            let invoice = Bolt11Invoice::from_str(&invoice)?;
            let amount = amount.map(Amount::from_sat);
            let node = state::get_node().inner.clone();

            match node.pay_invoice(&invoice, amount) {
                Ok(()) => tracing::info!("Successfully triggered payment"),
                Err(e) => {
                    // TODO(holzeis): This has been added to debug a users channel details in case
                    // of a failed payment. Remove the logs if not needed anymore.
                    for channel in node.channel_manager.list_channels().iter() {
                        tracing::debug!(
                            channel_id = channel.channel_id.to_hex(),
                            short_channel_id = channel.short_channel_id,
                            unspendable_punishment_reserve = channel.unspendable_punishment_reserve,
                            balance_msat = channel.balance_msat,
                            feerate_sat_per_1000_weight = channel.feerate_sat_per_1000_weight,
                            inbound_capacity_msat = channel.inbound_capacity_msat,
                            inbound_htlc_maximum_msat = channel.inbound_htlc_maximum_msat,
                            inbound_htlc_minimum_msat = channel.inbound_htlc_minimum_msat,
                            is_usable = channel.is_usable,
                            outbound_capacity_msat = channel.outbound_capacity_msat,
                            next_outbound_htlc_limit_msat = channel.next_outbound_htlc_limit_msat,
                            next_outbound_htlc_minimum_msat =
                                channel.next_outbound_htlc_minimum_msat,
                            is_channel_ready = channel.is_channel_ready,
                            "Channel Details"
                        );

                        let counterparty = channel.counterparty.clone();
                        tracing::debug!(
                            counterparty = %counterparty.node_id,
                            counterparty_unspendable_punishement_reserve =
                                counterparty.unspendable_punishment_reserve,
                            counterparty.outbound_htlc_maximum_msat,
                            counterparty.outbound_htlc_minimum_msat,
                            "Counterparty");

                        if let Some(forwarding_info) = counterparty.forwarding_info {
                            tracing::debug!(
                                forwarding_info.cltv_expiry_delta,
                                forwarding_info.fee_base_msat,
                                forwarding_info.fee_proportional_millionths,
                                "Forwarding info"
                            );
                        }

                        if let Some(config) = channel.config {
                            tracing::debug!(
                                config.cltv_expiry_delta,
                                config.forwarding_fee_base_msat,
                                config.forwarding_fee_proportional_millionths,
                                config.force_close_avoidance_max_fee_satoshis,
                                max_dust_htlc_exposure=?config.max_dust_htlc_exposure,
                                "Channel config"
                            )
                        }
                    }
                    tracing::error!("{e:#}");
                    bail!(e)
                }
            }
        }
        SendPayment::OnChain {
            address,
            amount,
            fee,
        } => {
            let address = Address::from_str(&address)?;
            state::get_node()
                .inner
                .send_to_address(&address, amount, fee.into())?;
        }
    }
    Ok(())
}

pub async fn estimate_payment_fee_msat(payment: SendPayment) -> Result<u64> {
    match payment {
        SendPayment::Lightning { invoice, amount } => {
            let invoice = Bolt11Invoice::from_str(&invoice)?;
            let amount = amount.map(Amount::from_sat);

            state::get_node()
                .inner
                .estimate_payment_fee_msat(invoice, amount, Duration::from_secs(10))
                .await
        }
        SendPayment::OnChain {
            address,
            amount,
            fee,
        } => {
            let address = address.parse()?;

            let fee = match fee {
                Fee::Priority(target) => state::get_node()
                    .inner
                    .calculate_fee(&address, amount, target.into())?
                    .to_sat(),
                Fee::Custom { sats } => sats,
            };

            Ok(fee * 1000)
        }
    }
}

pub async fn trade(trade_params: TradeParams) -> Result<(), (FailureReason, Error)> {
    let client = reqwest_client();
    let response = client
        .post(format!("http://{}/api/trade", config::get_http_endpoint()))
        .json(&trade_params)
        .send()
        .await
        .context("Failed to register with coordinator")
        .map_err(|e| (FailureReason::TradeRequest, e))?;

    if !response.status().is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };
        return Err((
            // TODO(bonomat): extract the error message
            FailureReason::TradeResponse,
            anyhow!("Could not post trade to coordinator: {response_text}"),
        ));
    }

    tracing::info!("Sent trade request to coordinator successfully");

    Ok(())
}

/// initiates the rollover protocol with the coordinator
pub async fn rollover(contract_id: Option<String>) -> Result<()> {
    let node = state::get_node();

    let dlc_channels = node.inner.dlc_manager.get_store().get_sub_channels()?;

    let dlc_channel = dlc_channels
        .into_iter()
        .find(|chan| {
            chan.counter_party == config::get_coordinator_info().pubkey
                && matches!(chan.state, SubChannelState::Signed(_))
        })
        .context("Couldn't find dlc channel to rollover")?;

    let dlc_channel_id = dlc_channel
        .get_dlc_channel_id(0)
        .context("Couldn't get dlc channel id")?;

    let channel = node
        .inner
        .dlc_manager
        .get_store()
        .get_channel(&dlc_channel_id)?;

    match channel {
        Some(rust_dlc_manager::channel::Channel::Signed(signed_channel)) => {
            let current_contract_id = signed_channel.get_contract_id().map(hex::encode);
            if current_contract_id != contract_id {
                bail!("Rejecting to rollover a contract that we are not aware of. Expected: {current_contract_id:?}, Got: {contract_id:?}");
            }
        }
        Some(channel) => {
            bail!("Found channel in unexpected state. Expected: Signed, Found: {channel:?}");
        }
        None => {
            bail!(
                "Couldn't find channel by dlc_channel_id: {}",
                hex::encode(dlc_channel_id)
            );
        }
    };

    let client = reqwest_client();
    let response = client
        .post(format!(
            "http://{}/api/rollover/{}",
            config::get_http_endpoint(),
            dlc_channel_id.to_hex()
        ))
        .send()
        .await
        .with_context(|| format!("Failed to rollover dlc with id {}", dlc_channel_id.to_hex()))?;

    if !response.status().is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };

        bail!(
            "Failed to rollover dlc with id {}. Error: {response_text}",
            dlc_channel_id.to_hex()
        )
    }

    tracing::info!("Sent rollover request to coordinator successfully");

    Ok(())
}

fn ln_dlc_node_settings() -> LnDlcNodeSettings {
    let gossip_source_config = match get_rgs_server_url() {
        Some(server_url) => GossipSourceConfig::RapidGossipSync { server_url },
        None => GossipSourceConfig::P2pNetwork,
    };

    LnDlcNodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        dlc_manager_periodic_check_interval: Duration::from_secs(30),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
        forwarding_fee_proportional_millionths: 50,
        bdk_client_stop_gap: 20,
        bdk_client_concurrency: 4,
        gossip_source_config,
    }
}
