use crate::api;
use crate::api::Fee;
use crate::api::PaymentFlow;
use crate::api::Status;
use crate::api::WalletHistoryItem;
use crate::api::WalletHistoryItemType;
use crate::backup::DBBackupSubscriber;
use crate::commons::reqwest_client;
use crate::config;
use crate::db;
use crate::dlc::dlc_handler;
use crate::dlc::dlc_handler::DlcHandler;
use crate::event;
use crate::event::EventInternal;
use crate::health::Tx;
use crate::ln_dlc::node::Node;
use crate::ln_dlc::node::NodeStorage;
use crate::ln_dlc::node::WalletHistory;
use crate::orderbook;
use crate::position::ForceCloseDlcChannelSubscriber;
use crate::state;
use crate::storage::TenTenOneNodeStorage;
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
use anyhow::Result;
use bdk::wallet::Balance;
use bdk::FeeRate;
use bitcoin::address::NetworkUnchecked;
use bitcoin::key::XOnlyPublicKey;
use bitcoin::secp256k1::rand::thread_rng;
use bitcoin::secp256k1::rand::RngCore;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::SECP256K1;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Txid;
use commons::CollaborativeRevertTraderResponse;
use commons::OrderbookRequest;
use dlc::PartyParams;
use dlc_manager::channel::Channel as DlcChannel;
use itertools::chain;
use itertools::Itertools;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::events::Event;
use lightning::ln::ChannelId;
use lightning::sign::KeysManager;
use ln_dlc_node::bitcoin_conversion::to_ecdsa_signature_30;
use ln_dlc_node::bitcoin_conversion::to_script_29;
use ln_dlc_node::bitcoin_conversion::to_secp_sk_30;
use ln_dlc_node::bitcoin_conversion::to_tx_30;
use ln_dlc_node::bitcoin_conversion::to_txid_29;
use ln_dlc_node::bitcoin_conversion::to_txid_30;
use ln_dlc_node::config::app_config;
use ln_dlc_node::node::dlc_channel::estimated_dlc_channel_fee_reserve;
use ln_dlc_node::node::dlc_channel::estimated_funding_transaction_fee;
use ln_dlc_node::node::event::NodeEventHandler;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannel;
use ln_dlc_node::node::rust_dlc_manager::channel::ClosedChannel;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::rust_dlc_manager::DlcChannelId;
use ln_dlc_node::node::rust_dlc_manager::Signer;
use ln_dlc_node::node::rust_dlc_manager::Storage as DlcStorage;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::seed::Bip39Seed;
use ln_dlc_node::AppEventHandler;
use ln_dlc_node::ConfirmationStatus;
use ln_dlc_storage::DlcChannelEvent;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use time::OffsetDateTime;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio::task::spawn_blocking;
use uuid::Uuid;

pub mod node;

mod lightning_subscriber;

const PROCESS_INCOMING_DLC_MESSAGES_INTERVAL: Duration = Duration::from_millis(200);
const UPDATE_WALLET_HISTORY_INTERVAL: Duration = Duration::from_secs(5);
const CHECK_OPEN_ORDERS_INTERVAL: Duration = Duration::from_secs(60);
const NODE_SYNC_INTERVAL: Duration = Duration::from_secs(300);

/// The name of the BDK wallet database file.
const WALLET_DB_FILE_NAME: &str = "bdk-wallet";

/// The prefix to the [`bdk_file_store`] database file where BDK persists
/// [`bdk::wallet::ChangeSet`]s.
///
/// We hard-code the prefix so that we can always be sure that we are loading the correct file on
/// start-up.
const WALLET_DB_PREFIX: &str = "10101-app";

/// Trigger an on-chain sync followed by an update to the wallet balance and history.
///
/// We do not wait for the triggered task to finish, because the effect will be reflected
/// asynchronously on the UI.
pub async fn refresh_wallet_info() -> Result<()> {
    let node = state::get_node();

    let runtime = state::get_or_create_tokio_runtime()?;

    sync_node(runtime.handle()).await;

    // Spawn into the blocking thread pool of the dedicated backend runtime to avoid blocking the UI
    // thread.
    runtime.spawn_blocking(move || {
        if let Err(e) = keep_wallet_balance_and_history_up_to_date(&node) {
            tracing::error!("Failed to keep wallet history up to date: {e:#}");
        }
    });

    Ok(())
}

pub async fn sync_node(runtime: &runtime::Handle) {
    let node = state::get_node();

    if let Err(e) = node.inner.sync_on_chain_wallet().await {
        tracing::error!("On-chain sync failed: {e:#}");
    }

    runtime
        .spawn_blocking(move || {
            if let Err(e) = node.inner.dlc_manager.periodic_check() {
                tracing::error!("Failed to run DLC manager periodic check: {e:#}");
            };
        })
        .await
        .expect("task to complete");
}

pub async fn full_sync(stop_gap: usize) -> Result<()> {
    let runtime = state::get_or_create_tokio_runtime()?;
    runtime
        .spawn({
            let node = state::get_node();
            async move {
                node.inner.full_sync(stop_gap).await?;
                anyhow::Ok(())
            }
        })
        .await
        .expect("task to complete")?;

    Ok(())
}

pub fn get_seed_phrase() -> Vec<String> {
    state::get_seed().get_seed_phrase()
}

pub fn get_maintenance_margin_rate() -> Decimal {
    match state::try_get_tentenone_config() {
        Some(config) => {
            Decimal::try_from(config.maintenance_margin_rate).expect("to fit into decimal")
        }
        None => {
            tracing::warn!("The ten ten one config is not ready yet. Returning default value!");
            dec!(0.1)
        }
    }
}

pub fn get_order_matching_fee_rate() -> Decimal {
    match state::try_get_tentenone_config() {
        Some(config) => {
            let fee_percent =
                Decimal::try_from(config.order_matching_fee_rate).expect("to fit into decimal");
            let fee_discount = config.referral_status.referral_fee_bonus;
            fee_percent - (fee_percent * fee_discount)
        }
        None => dec!(0.003),
    }
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
        // TODO: This seems pretty suspicious.
        None => {
            let seed = get_seed();
            let time_since_unix_epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("unix epos to not be earlier than now");
            let keys_manager = KeysManager::new(
                &seed.lightning_seed(),
                time_since_unix_epoch.as_secs(),
                time_since_unix_epoch.subsec_nanos(),
            );
            to_secp_sk_30(keys_manager.get_node_secret_key())
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

    let channel_details = channel_details.with_context(|| {
        format!(
            "Could not find channel details for {}",
            hex::encode(channel_id.0)
        )
    })?;

    let funding_txo = channel_details.funding_txo.with_context(|| {
        format!(
            "Could not find funding transaction for channel {}",
            hex::encode(channel_id.0)
        )
    })?;

    Ok(to_txid_30(funding_txo.txid))
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
/// Assumes that the seed has already been initialized
pub fn run(
    runtime: &Runtime,
    tx: Tx,
    fcm_token: String,
    tx_websocket: broadcast::Sender<OrderbookRequest>,
) -> Result<()> {
    runtime.block_on(async move {
        event::publish(&EventInternal::Init("Starting full ldk node".to_string()));

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0")?;
            listener.local_addr().expect("To get a free local address")
        };

        let (event_sender, event_receiver) = watch::channel::<Option<Event>>(None);

        let node_storage = Arc::new(NodeStorage);

        let storage = get_storage();

        event::subscribe(DBBackupSubscriber::new(storage.clone().client));
        event::subscribe(ForceCloseDlcChannelSubscriber);

        let node_event_handler = Arc::new(NodeEventHandler::new());

        let wallet_storage = {
            let wallet_dir = Path::new(&config::get_data_dir()).join(WALLET_DB_FILE_NAME);
            bdk_file_store::Store::open_or_create_new(WALLET_DB_PREFIX.as_bytes(), wallet_dir)?
        };

        let (dlc_event_sender, dlc_event_receiver) = mpsc::channel::<DlcChannelEvent>();
        let node = ln_dlc_node::node::Node::new(
            app_config(),
            "10101",
            config::get_network(),
            Path::new(&storage.data_dir),
            storage.clone(),
            node_storage,
            wallet_storage,
            address,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
            config::get_electrs_endpoint(),
            state::get_seed(),
            ephemeral_randomness,
            ln_dlc_node_settings(),
            vec![config::get_oracle_info().into()],
            config::get_oracle_info().public_key,
            node_event_handler.clone(),
            dlc_event_sender,
        )?;
        let node = Arc::new(node);

        let event_handler = AppEventHandler::new(node.clone(), Some(event_sender));
        let _running = node.start(event_handler, dlc_event_receiver, true)?;

        let node = Arc::new(Node::new(node, _running));
        state::set_node(node.clone());

        orderbook::subscribe(
            node.inner.node_key(),
            runtime,
            tx.orderbook,
            fcm_token,
            tx_websocket,
        )?;

        if let Err(e) = spawn_blocking({
            let node = node.clone();
            move || keep_wallet_balance_and_history_up_to_date(&node)
        })
        .await
        .expect("To spawn blocking task")
        {
            tracing::error!("Failed to update balance and history: {e:#}");
        }

        let dlc_handler = DlcHandler::new(node.clone());
        runtime.spawn(async move {
            dlc_handler::handle_outbound_dlc_messages(dlc_handler, node_event_handler.subscribe())
                .await
        });

        node.spawn_listen_dlc_channels_event_task();

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
                        tracing::error!("Failed to update balance and history: {e:#}");
                    }
                }
            }
        });

        runtime.spawn({
            let runtime = runtime.handle().clone();
            async move {
                loop {
                    sync_node(&runtime).await;

                    tokio::time::sleep(NODE_SYNC_INTERVAL).await;
                }
            }
        });

        runtime.spawn({
            let node = node.clone();
            async move { node.listen_for_lightning_events(event_receiver).await }
        });

        let coordinator_info = config::get_coordinator_info();
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

        event::publish(&EventInternal::Init("10101 is ready.".to_string()));

        tokio::spawn(full_sync_on_wallet_db_migration());

        Ok(())
    })
}

pub async fn full_sync_on_wallet_db_migration() {
    let node = state::get_node();

    let old_wallet_dir = Path::new(&config::get_data_dir())
        .join(config::get_network().to_string())
        .join("on_chain");
    if old_wallet_dir.exists() {
        event::publish(&EventInternal::BackgroundNotification(
            event::BackgroundTask::FullSync(event::TaskStatus::Pending),
        ));

        let stop_gap = 20;
        tracing::info!(
            %stop_gap,
            "Old wallet directory detected. Attempting to populate new wallet with full sync"
        );

        match node.inner.full_sync(stop_gap).await {
            Ok(_) => {
                tracing::info!("Full sync successful");

                // Spawn into the blocking thread pool of the dedicated backend runtime to avoid
                // blocking the UI thread.
                if let Ok(runtime) = state::get_or_create_tokio_runtime() {
                    runtime
                        .spawn_blocking(move || {
                            if let Err(e) = keep_wallet_balance_and_history_up_to_date(&node) {
                                tracing::error!("Failed to keep wallet history up to date: {e:#}");
                            }
                        })
                        .await
                        .expect("task to complete");
                }

                event::publish(&EventInternal::BackgroundNotification(
                    event::BackgroundTask::FullSync(event::TaskStatus::Success),
                ));

                if let Err(e) = std::fs::remove_dir_all(old_wallet_dir) {
                    tracing::info!("Failed to delete old wallet directory: {e:#}");
                }
            }
            Err(e) => {
                tracing::error!("Full sync failed: {e:#}");

                event::publish(&EventInternal::BackgroundNotification(
                    event::BackgroundTask::FullSync(event::TaskStatus::Failed),
                ));
            }
        };
    }
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
    let wallet_balances = node.get_wallet_balances();

    let WalletHistory { on_chain } = node.get_wallet_history();

    // If we find fund transactions among the on-chain transactions we are aware of, we treat them
    // as a special case so that they can be displayed with extra information.
    let dlc_channels = node.inner.list_signed_dlc_channels()?;
    let dlc_channel_funding_tx_details = on_chain.iter().filter_map(|details| {
        match dlc_channels
            .iter()
            .find(|item| item.fund_tx.txid() == to_txid_29(details.transaction.txid()))
        {
            None => None,
            Some(channel) => {
                let amount_sats = match details.sent.checked_sub(details.received) {
                    Some(amount_sats) => amount_sats,
                    None => {
                        tracing::warn!("Omitting DLC channel funding transaction that pays to us!");
                        return None;
                    }
                };

                let (status, timestamp) =
                    confirmation_status_to_status_and_timestamp(&details.confirmation_status);

                Some(WalletHistoryItem {
                    flow: PaymentFlow::Outbound,
                    amount_sats: amount_sats.to_sat(),
                    timestamp,
                    status,
                    wallet_type: WalletHistoryItemType::DlcChannelFunding {
                        funding_txid: details.transaction.txid().to_string(),
                        // this is not 100% correct as fees are not exactly divided by 2. The share
                        // of the funding transaction fee that the user has paid depends on their
                        // inputs and change outputs.
                        funding_tx_fee_sats: details
                            .fee
                            .as_ref()
                            .map(|fee| (*fee / 2).to_sat())
                            .ok(),
                        confirmations: details.confirmation_status.n_confirmations() as u64,
                        our_channel_input_amount_sats: channel.own_params.collateral,
                    },
                })
            }
        }
    });

    let on_chain = on_chain.iter().filter(|details| {
        !dlc_channels
            .iter()
            .any(|channel| channel.fund_tx.txid() == to_txid_29(details.transaction.txid()))
    });

    let on_chain = on_chain.filter_map(|details| {
        let net_sats = match details.net_amount() {
            Ok(net_amount) => net_amount.to_sat(),
            Err(e) => {
                tracing::error!(
                    ?details,
                    "Failed to calculate net amount for transaction: {e:#}"
                );
                return None;
            }
        };

        let (flow, amount_sats) = if net_sats >= 0 {
            (PaymentFlow::Inbound, net_sats as u64)
        } else {
            (PaymentFlow::Outbound, net_sats.unsigned_abs())
        };

        let (status, timestamp) =
            confirmation_status_to_status_and_timestamp(&details.confirmation_status);

        let wallet_type = WalletHistoryItemType::OnChain {
            txid: details.transaction.txid().to_string(),
            fee_sats: details.fee.as_ref().map(|fee| Amount::to_sat(*fee)).ok(),
            confirmations: details.confirmation_status.n_confirmations() as u64,
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

    let history = chain![on_chain, trades, dlc_channel_funding_tx_details]
        .sorted_by(|a, b| b.timestamp.cmp(&a.timestamp))
        .collect();

    let wallet_info = api::WalletInfo {
        balances: wallet_balances.into(),
        history,
    };

    event::publish(&EventInternal::WalletInfoUpdateNotification(wallet_info));

    Ok(())
}

pub fn get_unused_address() -> Result<String> {
    let address = state::get_node().inner.get_unused_address()?;

    Ok(address.to_string())
}

pub fn get_new_address() -> Result<String> {
    let address = state::get_node().inner.get_new_address()?;

    Ok(address.to_string())
}

pub async fn close_channel(is_force_close: bool) -> Result<()> {
    let node = state::get_node();

    let channels = node.inner.list_signed_dlc_channels()?;
    let channel_details = channels.first().context("No channel to close")?;

    node.inner
        .close_dlc_channel(channel_details.channel_id, is_force_close)
        .await
}

pub fn get_signed_dlc_channels() -> Result<Vec<SignedChannel>> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(vec![]),
    };
    node.inner.list_signed_dlc_channels()
}

pub fn get_onchain_balance() -> Balance {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Balance::default(),
    };
    node.inner.get_on_chain_balance()
}

pub fn get_usable_dlc_channel_balance() -> Result<Amount> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(Amount::ZERO),
    };
    node.inner.get_dlc_channels_usable_balance()
}

pub fn get_usable_dlc_channel_balance_counterparty() -> Result<Amount> {
    let node = state::get_node();
    node.inner.get_dlc_channels_usable_balance_counterparty()
}

pub fn collaborative_revert_channel(
    channel_id: DlcChannelId,
    coordinator_address: Address<NetworkUnchecked>,
    coordinator_amount: Amount,
    trader_amount: Amount,
    execution_price: Decimal,
) -> Result<()> {
    let node = state::get_node();
    let node = node.inner.clone();

    let coordinator_address = coordinator_address.require_network(node.network)?;

    let channel_id_hex = hex::encode(channel_id);
    let dlc_channels = node.list_signed_dlc_channels()?;

    let signed_channel = dlc_channels
        .into_iter()
        .find(|c| c.channel_id == channel_id)
        .with_context(|| format!("Could not find signed channel {channel_id_hex}"))?;

    let fund_output_value = signed_channel.fund_tx.output[signed_channel.fund_output_index].value;

    tracing::debug!(
        channel_id = channel_id_hex,
        trader_amount_sats = %trader_amount.to_sat(),
        coordinator_amount_sats = %coordinator_amount.to_sat(),
        "Accepting collaborative revert request");

    let close_tx = dlc::channel::create_collaborative_close_transaction(
        &PartyParams {
            payout_script_pubkey: to_script_29(coordinator_address.script_pubkey()),
            ..signed_channel.counter_params.clone()
        },
        coordinator_amount.to_sat(),
        &signed_channel.own_params,
        trader_amount.to_sat(),
        bitcoin_old::OutPoint {
            txid: signed_channel.fund_tx.txid(),
            vout: signed_channel.fund_output_index as u32,
        },
        0, // argument is not being used
    );

    let own_fund_sk = node
        .dlc_wallet
        .get_secret_key_for_pubkey(&signed_channel.own_params.fund_pubkey)?;

    let close_signature = dlc::util::get_raw_sig_for_tx_input(
        &bitcoin_old::secp256k1::Secp256k1::new(),
        &close_tx,
        0,
        &signed_channel.fund_script_pubkey,
        fund_output_value,
        &own_fund_sk,
    )?;
    tracing::debug!(
        tx_id = close_tx.txid().to_string(),
        "Signed collab revert transaction"
    );

    let data = CollaborativeRevertTraderResponse {
        channel_id: channel_id_hex,
        transaction: to_tx_30(close_tx.clone()),
        signature: to_ecdsa_signature_30(close_signature),
    };

    let client = reqwest_client();
    let runtime = state::get_or_create_tokio_runtime()?;
    runtime.spawn({

        async move {
            match client
                .post(format!(
                    "http://{}/api/channels/confirm-collab-revert",
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
                            update_state_after_collab_revert(&signed_channel, execution_price, to_txid_30(close_tx.txid()))
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
    signed_channel: &SignedChannel,
    execution_price: Decimal,
    closing_txid: Txid,
) -> Result<()> {
    let node = state::get_node();
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
                id: Uuid::new_v4(),
                leverage: position.leverage,
                quantity: position.quantity,
                contract_symbol: position.contract_symbol,
                direction: position.direction.opposite(),
                order_type: OrderType::Market,
                state: OrderState::Filled {
                    execution_price: execution_price.to_f32().expect("to fit into f32"),
                    // this fee here doesn't matter because it's not being used anywhere
                    matching_fee: Amount::ZERO,
                },
                creation_timestamp: OffsetDateTime::now_utc(),
                order_expiry_timestamp: OffsetDateTime::now_utc(),
                reason: OrderReason::Expired,
                stable: position.stable,
                failure_reason: None,
            };
            db::insert_order(order.clone())?;
            event::publish(&EventInternal::OrderUpdateNotification(order.clone()));
            order
        }
    };

    position::handler::update_position_after_dlc_closure(Some(filled_order))?;

    let node = node.inner.clone();

    node.dlc_manager
        .get_store()
        .upsert_channel(
            dlc_manager::channel::Channel::CollaborativelyClosed(ClosedChannel {
                counter_party: signed_channel.counter_party,
                temporary_channel_id: signed_channel.temporary_channel_id,
                channel_id: signed_channel.channel_id,
                reference_id: None,
                closing_txid: to_txid_29(closing_txid),
            }),
            // The contract doesn't matter anymore
            None,
        )
        .map_err(|e| anyhow!("{e:#}"))
}

pub fn get_signed_dlc_channel() -> Result<Option<SignedChannel>> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(None),
    };

    let signed_channels = node.inner.list_signed_dlc_channels()?;
    Ok(signed_channels.first().cloned())
}

pub fn list_dlc_channels() -> Result<Vec<DlcChannel>> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(vec![]),
    };

    let dlc_channels = node.inner.list_dlc_channels()?;

    Ok(dlc_channels)
}

pub fn delete_dlc_channel(dlc_channel_id: &DlcChannelId) -> Result<()> {
    let node = state::get_node();
    node.inner
        .dlc_manager
        .get_store()
        .delete_channel(dlc_channel_id)?;

    Ok(())
}

pub fn is_dlc_channel_confirmed() -> Result<bool> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(false),
    };

    let dlc_channel = match get_signed_dlc_channel()? {
        Some(dlc_channel) => dlc_channel,
        None => return Ok(false),
    };

    node.inner.is_dlc_channel_confirmed(&dlc_channel.channel_id)
}

pub fn get_fee_rate_for_target(target: ConfirmationTarget) -> FeeRate {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return FeeRate::default_min_relay_fee(),
    };
    node.inner.fee_rate_estimator.get(target)
}

pub fn estimated_fee_reserve() -> Result<Amount> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(Amount::ZERO),
    };

    // Here we assume that the coordinator will use the same confirmation target AND that their fee
    // rate source agrees with ours.
    let fee_rate = node
        .inner
        .fee_rate_estimator
        .get(ConfirmationTarget::Normal);

    let reserve = estimated_dlc_channel_fee_reserve(fee_rate.as_sat_per_vb() as f64);

    // The reserve is split evenly between the two parties.
    let reserve = reserve / 2;

    Ok(reserve)
}

pub async fn send_payment(amount: u64, address: String, fee: Fee) -> Result<Txid> {
    let address = Address::from_str(&address)?;

    let txid = state::get_node()
        .inner
        .send_to_address(address, amount, fee.into())
        .await?;

    Ok(txid)
}

pub fn estimated_funding_tx_fee() -> Result<Amount> {
    let node = match state::try_get_node() {
        Some(node) => node,
        None => return Ok(Amount::ZERO),
    };

    // Here we assume that the coordinator will use the same confirmation target AND that
    // their fee rate source agrees with ours.
    let fee_rate = node
        .inner
        .fee_rate_estimator
        .get(ConfirmationTarget::Normal);

    let fee = estimated_funding_transaction_fee(fee_rate.as_sat_per_vb() as f64);

    // The estimated fee is split evenly between the two parties. In reality, each party will have
    // to pay more or less depending on their inputs and change outputs.
    let fee = fee / 2;

    Ok(fee)
}

/// Returns true if the provided address belongs to our wallet and false otherwise.
/// Returns and error if the address is invalid.
///
/// Note, this may return false if the wallet doesn't know about the address. A full sync is
/// required then.
pub fn is_address_mine(address: &str) -> Result<bool> {
    let address: Address<NetworkUnchecked> = address.parse().context("Failed to parse address")?;
    let is_mine = state::get_node()
        .inner
        .is_mine(&address.payload.script_pubkey());
    Ok(is_mine)
}

pub async fn estimate_payment_fee(amount: u64, address: &str, fee: Fee) -> Result<Option<Amount>> {
    let address: Address<NetworkUnchecked> = address.parse().context("Failed to parse address")?;
    // This is safe to do because we are only using this address to estimate a fee.
    let address = address.assume_checked();

    let fee = match fee {
        Fee::Priority(target) => {
            match state::get_node()
                .inner
                .estimate_fee(address, amount, target.into())
            {
                Ok(fee) => Some(fee),
                // It's not sensible to calculate the fee for an amount below dust.
                Err(ln_dlc_node::EstimateFeeError::SendAmountBelowDust) => None,
                Err(e) => {
                    bail!("Failed to estimate payment fee: {e:#}")
                }
            }
        }
        Fee::FeeRate { sats } => Some(Amount::from_sat(sats)),
    };

    Ok(fee)
}

fn ln_dlc_node_settings() -> LnDlcNodeSettings {
    LnDlcNodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
    }
}

fn confirmation_status_to_status_and_timestamp(
    confirmation_status: &ConfirmationStatus,
) -> (Status, u64) {
    let (status, timestamp) = match confirmation_status {
        ConfirmationStatus::Confirmed { timestamp, .. } => (Status::Confirmed, *timestamp),
        // Unfortunately, the `last_seen` we get from BDK seems to be unreliable. At least on
        // regtest, it can be 0, which is not a very useful UNIX timestamp.
        ConfirmationStatus::Mempool { last_seen: _ } => (
            Status::Pending,
            // Unconfirmed transactions should appear towards the top of the history.
            OffsetDateTime::now_utc(),
        ),
        ConfirmationStatus::Unknown => {
            (
                Status::Pending,
                // Unconfirmed transactions should appear towards the top of the history.
                OffsetDateTime::now_utc(),
            )
        }
    };

    (status, timestamp.unix_timestamp() as u64)
}

pub fn roll_back_channel_state() -> Result<()> {
    let node = state::get_node();

    let counterparty_pubkey = config::get_coordinator_info().pubkey;
    let signed_channel = node
        .inner
        .get_signed_channel_by_trader_id(counterparty_pubkey)?;

    node.inner.roll_back_channel(&signed_channel)
}
