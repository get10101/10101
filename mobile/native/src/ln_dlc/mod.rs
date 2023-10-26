use self::node::WalletHistories;
use crate::api;
use crate::api::PaymentFlow;
use crate::api::SendPayment;
use crate::api::Status;
use crate::api::WalletHistoryItem;
use crate::api::WalletHistoryItemType;
use crate::calculations;
use crate::channel_fee::ChannelFeePaymentSubscriber;
use crate::commons::reqwest_client;
use crate::config;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::channel_status::track_channel_status;
use crate::ln_dlc::node::Node;
use crate::ln_dlc::node::NodeStorage;
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
use bdk::BlockTime;
use bdk::FeeRate;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::PackedLockTime;
use bitcoin::Transaction;
use bitcoin::TxIn;
use bitcoin::TxOut;
pub use channel_status::ChannelStatus;
use coordinator_commons::CollaborativeRevertData;
use coordinator_commons::LiquidityOption;
use coordinator_commons::LspConfig;
use coordinator_commons::OnboardingParam;
use coordinator_commons::TradeParams;
use itertools::chain;
use itertools::Itertools;
use lightning::chain::keysinterface::ExtraSign;
use lightning::chain::keysinterface::SignerProvider;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::events::Event;
use lightning_invoice::Invoice;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::channel::UserChannelId;
use ln_dlc_node::channel::JIT_FEE_INVOICE_DESCRIPTION_PREFIX;
use ln_dlc_node::config::app_config;
use ln_dlc_node::node::rust_dlc_manager;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannel;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannelState;
use ln_dlc_node::node::rust_dlc_manager::ChannelId;
use ln_dlc_node::node::rust_dlc_manager::Storage as DlcStorage;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::Storage as LnDlcNodeStorage;
use ln_dlc_node::scorer;
use ln_dlc_node::seed::Bip39Seed;
use ln_dlc_node::util;
use ln_dlc_node::AppEventHandler;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::CONFIRMATION_TARGET;
use orderbook_commons::order_matching_fee_taker;
use orderbook_commons::RouteHintHop;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::Signed;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use state::Storage;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio::task::spawn_blocking;
use trade::ContractSymbol;

mod lightning_subscriber;
mod node;
mod sync_position_to_dlc;

pub mod channel_status;
mod recover_rollover;

const PROCESS_INCOMING_DLC_MESSAGES_INTERVAL: Duration = Duration::from_millis(200);
const UPDATE_WALLET_HISTORY_INTERVAL: Duration = Duration::from_secs(5);
const CHECK_OPEN_ORDERS_INTERVAL: Duration = Duration::from_secs(60);
const ON_CHAIN_SYNC_INTERVAL: Duration = Duration::from_secs(300);

/// The weight estimate of the funding transaction
///
/// This weight estimate assumes two inputs.
/// This value was chosen based on mainnet channel funding transactions with two inputs.
/// Note that we cannot predict this value precisely, because the app cannot predict what UTXOs the
/// coordinator will use for the channel opening transaction. Only once the transaction is know the
/// exact fee will be know.
pub const FUNDING_TX_WEIGHT_ESTIMATE: u64 = 220;

static NODE: Storage<Arc<Node>> = Storage::new();
static SEED: Storage<Bip39Seed> = Storage::new();

/// Trigger an on-chain sync followed by an update to the wallet balance and history.
///
/// We do not wait for the triggered task to finish, because the effect will be reflected
/// asynchronously on the UI.
pub async fn refresh_wallet_info() -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let wallet = node.inner.wallet();

    // Spawn into the blocking thread pool of the dedicated backend runtime to avoid blocking the UI
    // thread.
    let runtime = get_or_create_tokio_runtime()?;
    runtime.spawn_blocking(move || {
        if let Err(e) = wallet.sync() {
            tracing::error!("Manually triggered on-chain sync failed: {e:#}");
        }

        if let Err(e) = node.inner.sync_lightning_wallet() {
            tracing::error!("Manually triggered Lightning wallet sync failed: {e:#}");
        }

        if let Err(e) = keep_wallet_balance_and_history_up_to_date(node) {
            tracing::error!("Failed to keep wallet history up to date: {e:#}");
        }

        anyhow::Ok(())
    });

    Ok(())
}

pub fn get_seed_phrase() -> Vec<String> {
    SEED.try_get()
        .expect("SEED to be initialised")
        .get_seed_phrase()
}

pub fn get_node_key() -> SecretKey {
    NODE.get().inner.node_key()
}

pub fn get_node_info() -> Result<NodeInfo> {
    Ok(NODE
        .try_get()
        .context("NODE is not initialised yet, can't retrieve node info")?
        .inner
        .info)
}

pub async fn update_node_settings(settings: LnDlcNodeSettings) {
    let node = NODE.get();
    node.inner.update_settings(settings).await;
}

pub fn get_oracle_pubkey() -> XOnlyPublicKey {
    NODE.get().inner.oracle_pk()
}

pub fn get_funding_transaction(channel_id: &ChannelId) -> Result<Txid> {
    let node = NODE.get();
    let channel_details = node.inner.channel_manager.get_channel_details(channel_id);

    let funding_transaction = match channel_details {
        Some(channel_details) => match channel_details.funding_txo {
            Some(funding_txo) => funding_txo.txid,
            None => bail!(
                "Could not find funding transaction for channel {}",
                hex::encode(channel_id)
            ),
        },
        None => bail!(
            "Could not find channel details for {}",
            hex::encode(channel_id)
        ),
    };

    Ok(funding_transaction)
}

/// Lazily creates a multi threaded runtime with the the number of worker threads corresponding to
/// the number of available cores.
pub fn get_or_create_tokio_runtime() -> Result<&'static Runtime> {
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}

/// Start the node
///
/// Allows specifying a data directory and a seed directory to decouple
/// data and seed storage (e.g. data is useful for debugging, seed location
/// should be more protected).
pub fn run(data_dir: String, seed_dir: String, runtime: &Runtime) -> Result<()> {
    let network = config::get_network();

    runtime.block_on(async move {
        event::publish(&EventInternal::Init("Starting full ldk node".to_string()));

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let data_dir = Path::new(&data_dir).join(network.to_string());
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        // TODO: Consider using the same seed dir for all networks, and instead
        // change the filename, e.g. having `mainnet-seed` or `regtest-seed`
        let seed_dir = Path::new(&seed_dir).join(network.to_string());
        if !seed_dir.exists() {
            std::fs::create_dir_all(&seed_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        event::subscribe(position::subscriber::Subscriber {});
        // TODO: Subscribe to events from the orderbook and publish OrderFilledWith event

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0")?;
            listener.local_addr().expect("To get a free local address")
        };

        let seed_path = seed_dir.join("seed");
        let seed = Bip39Seed::initialize(&seed_path)?;
        SEED.set(seed.clone());

        let (event_sender, event_receiver) = watch::channel::<Option<Event>>(None);

        let node = ln_dlc_node::node::Node::new(
            app_config(),
            scorer::in_memory_scorer,
            "10101",
            network,
            data_dir.as_path(),
            Arc::new(NodeStorage),
            address,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
            util::into_net_addresses(address),
            config::get_esplora_endpoint(),
            seed,
            ephemeral_randomness,
            LnDlcNodeSettings::default(),
            config::get_oracle_info().into(),
        )?;
        let node = Arc::new(node);

        let event_handler = AppEventHandler::new(node.clone(), Some(event_sender));
        let _running = node.start(event_handler)?;
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

        runtime.spawn({
            let node = node.clone();
            async move { node.keep_connected(config::get_coordinator_info()).await }
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

        event::subscribe(ChannelFeePaymentSubscriber::new(
            node.inner.channel_manager.clone(),
        ));

        runtime.spawn(track_channel_status(node.clone()));

        if let Err(e) = node.sync_position_with_dlc_channel_state().await {
            tracing::error!("Failed to sync position with dlc channel state. Error: {e:#}");
        }

        if let Err(e) = node.recover_rollover().await {
            tracing::error!(
                "Failed to check and recover from a stuck rollover state. Error: {e:#}"
            );
        }

        NODE.set(node);

        event::publish(&EventInternal::Init("10101 is ready.".to_string()));

        Ok(())
    })
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
    let on_chain = on_chain.iter().map(|details| {
        let net_sats = details.received as i64 - details.sent as i64;

        let (flow, amount_sats) = if net_sats >= 0 {
            (PaymentFlow::Inbound, net_sats as u64)
        } else {
            (PaymentFlow::Outbound, net_sats.unsigned_abs())
        };

        let (timestamp, n_confirmations) = match details.confirmation_time {
            Some(BlockTime { timestamp, height }) => (
                timestamp,
                // This is calculated manually to avoid wasteful requests to esplora,
                // since we can just cache the blockchain height as opposed to fetching it for each
                // block as with `LnDlcWallet::get_transaction_confirmations`
                blockchain_height
                    .checked_sub(height as u64)
                    .unwrap_or_default(),
            ),

            None => {
                (
                    // Unconfirmed transactions should appear towards the top of the history
                    OffsetDateTime::now_utc().unix_timestamp() as u64,
                    0,
                )
            }
        };

        let status = if n_confirmations >= 3 {
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

        let decoded_invoice = match details.invoice.as_deref().map(Invoice::from_str) {
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

        let description = &details.description;
        let wallet_type = if let Some(funding_txid) =
            description.strip_prefix(JIT_FEE_INVOICE_DESCRIPTION_PREFIX)
        {
            WalletHistoryItemType::JitChannelFee {
                funding_txid: funding_txid.to_string(),
                payment_hash,
            }
        } else {
            let expiry_timestamp = decoded_invoice
                .and_then(|inv| inv.timestamp().checked_add(inv.expiry_time()))
                .map(|time| OffsetDateTime::from(time).unix_timestamp() as u64);

            WalletHistoryItemType::Lightning {
                payment_hash,
                description: details.description.clone(),
                payment_preimage: details.preimage.clone(),
                invoice: details.invoice.clone(),
                fee_msat: details.fee_msat,
                expiry_timestamp,
            }
        };

        Some(WalletHistoryItem {
            flow,
            amount_sats,
            timestamp,
            status,
            wallet_type,
        })
    });

    let trades = derive_trades_from_filled_orders()?;

    let history = chain![on_chain, off_chain, trades]
        .sorted_by(|a, b| b.timestamp.cmp(&a.timestamp))
        .collect();

    let wallet_info = api::WalletInfo {
        balances: wallet_balances.into(),
        history,
    };

    event::publish(&EventInternal::WalletInfoUpdateNotification(wallet_info));

    Ok(())
}

fn derive_trades_from_filled_orders() -> Result<Vec<WalletHistoryItem>> {
    let mut trades = vec![];
    let orders =
        crate::db::get_filled_orders().context("Failed to get filled orders; skipping update")?;

    match orders.as_slice() {
        [first, tail @ ..] => {
            // The first filled order must be an outbound "payment", since coins need to leave the
            // Lightning wallet to open the first DLC channel.
            let flow = PaymentFlow::Outbound;
            let amount_sats = first
                .trader_margin()
                .expect("Filled order to have a margin");

            let execution_price = Decimal::try_from(
                first
                    .execution_price()
                    .context("execution price to be set on a filled order")?,
            )?;
            let fee = order_matching_fee_taker(first.quantity, execution_price).to_sat();
            let amount_sats = amount_sats + fee;

            trades.push(WalletHistoryItem {
                flow,
                amount_sats,
                timestamp: first.creation_timestamp.unix_timestamp() as u64,
                status: Status::Confirmed, // TODO: Support other order/trade statuses
                wallet_type: WalletHistoryItemType::Trade {
                    order_id: first.id.to_string(),
                    fee_sat: fee,
                    pnl: None,
                },
            });

            let mut total_contracts = compute_relative_contracts(first);
            let mut previous_order = first;
            for order in tail {
                let new_contracts = compute_relative_contracts(order);
                let updated_total_contracts = total_contracts + new_contracts;

                let execution_price = Decimal::try_from(
                    order
                        .execution_price()
                        .context("execution price to be set on a filled order")?,
                )?;

                // Closing the position.
                if updated_total_contracts.is_zero() {
                    let open_order = previous_order;
                    let trader_margin = open_order
                        .trader_margin()
                        .expect("Filled order to have a margin");

                    let opening_price = open_order
                        .execution_price()
                        .expect("initial execution price to be set on a filled order");

                    let pnl = calculations::calculate_pnl(
                        opening_price,
                        trade::Price {
                            ask: execution_price,
                            bid: execution_price,
                        },
                        open_order.quantity,
                        open_order.leverage,
                        open_order.direction,
                    )?;

                    let close_position_fee =
                        order_matching_fee_taker(order.quantity, execution_price).to_sat();

                    // Closing a position is an inbound "payment", because the DLC channel is closed
                    // into the Lightning channel.
                    let flow = PaymentFlow::Inbound;
                    let amount_sats = (trader_margin as i64 + pnl) as u64;

                    trades.push(WalletHistoryItem {
                        flow,
                        amount_sats: amount_sats - close_position_fee,
                        timestamp: order.creation_timestamp.unix_timestamp() as u64,
                        status: Status::Confirmed,
                        wallet_type: WalletHistoryItemType::Trade {
                            order_id: order.id.to_string(),
                            fee_sat: close_position_fee,
                            pnl: Some(pnl),
                        },
                    });
                }
                // Opening the position.
                else if total_contracts.is_zero() && !updated_total_contracts.is_zero() {
                    // Opening a position is an outbound "payment", since coins need to leave the
                    // Lightning wallet to open a DLC channel.
                    let flow = PaymentFlow::Outbound;
                    let amount_sats = order
                        .trader_margin()
                        .expect("Filled order to have a margin");

                    let open_positions_fee =
                        order_matching_fee_taker(order.quantity, execution_price).to_sat();
                    let amount_sats = amount_sats + open_positions_fee;

                    trades.push(WalletHistoryItem {
                        flow,
                        amount_sats,
                        timestamp: order.creation_timestamp.unix_timestamp() as u64,
                        status: Status::Confirmed, // TODO: Support other order/trade statuses
                        wallet_type: WalletHistoryItemType::Trade {
                            order_id: order.id.to_string(),
                            fee_sat: open_positions_fee,
                            pnl: None,
                        },
                    });
                } else if total_contracts.signum() == updated_total_contracts.signum()
                    && updated_total_contracts.abs() > total_contracts.abs()
                {
                    debug_assert!(false, "extending the position is unimplemented");
                } else if total_contracts.signum() == updated_total_contracts.signum()
                    && updated_total_contracts.abs() < total_contracts.abs()
                {
                    debug_assert!(false, "reducing the position is unimplemented");
                } else {
                    // Changing position direction e.g. from 100 long to 50 short.
                    debug_assert!(false, "changing position direction is unimplemented");
                }

                total_contracts = updated_total_contracts;
                previous_order = order;
            }
        }
        [] => {
            // No trades.
        }
    }

    Ok(trades)
}

/// Compute the number of contracts for the [`Order`] relative to its [`Direction`].
fn compute_relative_contracts(order: &Order) -> Decimal {
    let contracts = Decimal::from_f32(order.quantity)
        .expect("quantity to fit into Decimal")
        // We round to 2 decimal places to avoid slight differences between opening and
        // closing orders.
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

    use trade::Direction::*;
    match order.direction {
        Long => contracts,
        Short => -contracts,
    }
}

pub fn get_unused_address() -> String {
    NODE.get().inner.get_unused_address().to_string()
}

pub fn close_channel(is_force_close: bool) -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;

    let channels = node.inner.list_channels();
    let channel_details = channels.first().context("No channel to close")?;

    node.inner
        .close_channel(channel_details.channel_id, is_force_close)?;

    Ok(())
}

pub fn collaborative_revert_channel(
    channel_id: ChannelId,
    coordinator_address: Address,
    coordinator_amount: Amount,
    trader_amount: Amount,
    execution_price: Decimal,
    outpoint: OutPoint,
) -> Result<()> {
    tracing::info!(
        txid = outpoint.txid.to_string(),
        channel_id = channel_id.to_hex(),
        "Confirming collaborative revert"
    );

    let node = NODE.try_get().context("failed to get ln dlc node")?;

    let node = node.inner.clone();

    let sub_channels = node.list_dlc_channels()?;
    let subchannel = sub_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Could not find provided channel")?;

    let address = node.get_unused_address();

    let collab_revert_tx = Transaction {
        version: 2,
        lock_time: PackedLockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: outpoint.txid,
                vout: outpoint.vout,
            },
            script_sig: Default::default(),
            sequence: Default::default(),
            witness: Default::default(),
        }],
        output: vec![
            TxOut {
                value: coordinator_amount.to_sat(),
                script_pubkey: coordinator_address.script_pubkey(),
            },
            TxOut {
                value: trader_amount.to_sat(),
                script_pubkey: address.script_pubkey(),
            },
        ],
    };

    let channel_value = subchannel.fund_value_satoshis;

    let monitor = node
        .chain_monitor
        .get_monitor(lightning::chain::transaction::OutPoint {
            txid: outpoint.txid,
            index: outpoint.vout as u16,
        })
        .map_err(|_| anyhow!("Could not get chain monitor"))?;
    let channel_monitor = monitor.deref();
    let user_channel_keys = channel_monitor
        .inner
        .lock()
        .map_err(|_| anyhow!("Could not acquire channel monitor lock"))?
        .channel_keys_id;

    let signer = node
        .keys_manager
        .derive_channel_signer(channel_value, user_channel_keys);

    let mut own_sig = None;
    signer.sign_with_fund_key_callback(&mut |key| {
        own_sig = Some(
            dlc::util::get_raw_sig_for_tx_input(
                &Secp256k1::new(),
                &collab_revert_tx,
                0,
                &subchannel.original_funding_redeemscript,
                channel_value,
                key,
            )
            .expect("To be able to get raw sig for tx input"),
        );
    });

    if let Some(own_sig) = own_sig {
        let data = CollaborativeRevertData {
            channel_id: hex::encode(subchannel.channel_id),
            transaction: collab_revert_tx,
            signature: own_sig,
        };

        let client = reqwest_client();
        let runtime = get_or_create_tokio_runtime()?;
        runtime.spawn({
            let sub_channel = subchannel.clone();
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
                                update_state_after_collab_revert(sub_channel, execution_price)
                            {
                                tracing::error!(
                                    "Failed to update state after collab revert. {e:#}"
                                );
                            }
                        }
                        Err(error) => {
                            tracing::error!("Failed at confirming reverting a channel {error:#}");
                        }
                    },
                    Err(err) => {
                        tracing::error!("Could not confirm collaborative revert {err:#}");
                    }
                }
            }
        });
    }

    Ok(())
}

fn update_state_after_collab_revert(
    sub_channel: SubChannel,
    execution_price: Decimal,
) -> Result<()> {
    let positions = db::get_positions()?;

    let position = match positions.first() {
        Some(position) => {
            tracing::info!("Channel is reverted before the position got closed successfully.");
            position
        }
        None => {
            tracing::info!("Channel is reverted before the position got opened successfully.");
            if let Some(order) = db::maybe_get_order_in_filling()? {
                order::handler::order_failed(
                    Some(order.id),
                    FailureReason::ProposeDlcChannel,
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

    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let node = node.inner.clone();

    node.dlc_manager
        .get_store()
        .upsert_sub_channel(&sub_channel)
        .map_err(|e| anyhow!("{e:#}"))
}

pub fn get_usable_channel_details() -> Result<Vec<ChannelDetails>> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    let channels = node.inner.list_usable_channels();

    Ok(channels)
}

pub fn get_fee_rate() -> Result<FeeRate> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    Ok(node.inner.wallet().get_fee_rate(CONFIRMATION_TARGET))
}

/// Returns channel value or zero if there is no channel yet.
///
/// This is used when checking max tradeable amount
pub fn max_channel_value() -> Result<Amount> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
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

// TODO(holzeis): We might want to consider caching the lsp config, as this shouldn't change too
// often and even if, I guess we can live with the user having to restart to get the newest configs?
fn fetch_lsp_config() -> Result<LspConfig, Error> {
    let runtime = get_or_create_tokio_runtime()?;
    runtime.block_on(async {
        let client = reqwest_client();
        let response = client
            .get(format!(
                "http://{}/api/lsp/config",
                config::get_http_endpoint(),
            ))
            // timeout arbitrarily chosen
            .timeout(Duration::from_secs(3))
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            bail!("Failed to fetch channel config from LSP: {text}")
        }

        let channel_config: LspConfig = response.json().await?;

        Ok(channel_config)
    })
}

pub fn contract_tx_fee_rate() -> Result<u64> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;
    if let Some(fee_rate_per_vb) = node
        .inner
        .list_dlc_channels()?
        .first()
        .map(|c| c.fee_rate_per_vb)
    {
        Ok(fee_rate_per_vb)
    } else {
        let lsp_config = fetch_lsp_config()?;
        tracing::info!(
            contract_tx_fee_rate = lsp_config.contract_tx_fee_rate,
            "Received channel config from LSP"
        );
        Ok(lsp_config.contract_tx_fee_rate)
    }
}

pub fn liquidity_options() -> Result<Vec<LiquidityOption>> {
    let lsp_config = fetch_lsp_config()?;
    tracing::trace!(liquidity_options=?lsp_config.liquidity_options, "Received liquidity options");
    Ok(lsp_config.liquidity_options)
}

pub fn create_onboarding_invoice(amount_sats: u64, liquidity_option_id: i32) -> Result<Invoice> {
    let runtime = get_or_create_tokio_runtime()?;

    runtime.block_on(async {
        let node = NODE.get();
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
                    liquidity_option_id
                );
                node.inner
                    .storage
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

pub fn create_invoice(amount_sats: Option<u64>) -> Result<Invoice> {
    let node = NODE.get();

    let final_route_hint_hop = node
        .inner
        .prepare_payment_with_route_hint(config::get_coordinator_info().pubkey)?;

    node.inner.create_invoice_with_route_hint(
        amount_sats,
        None,
        "".to_string(),
        final_route_hint_hop,
    )
}

pub fn send_payment(payment: SendPayment) -> Result<()> {
    match payment {
        SendPayment::Lightning { invoice, amount } => {
            let invoice = Invoice::from_str(&invoice)?;
            NODE.get().inner.pay_invoice(&invoice, amount)?;
        }
        SendPayment::OnChain { address, amount } => {
            let address = Address::from_str(&address)?;
            NODE.get().inner.send_to_address(&address, amount)?;
        }
    }
    Ok(())
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
            FailureReason::TradeResponse,
            anyhow!("Could not post trade to coordinator: {response_text}"),
        ));
    }

    tracing::info!("Sent trade request to coordinator successfully");

    Ok(())
}

/// initiates the rollover protocol with the coordinator
pub async fn rollover(contract_id: Option<String>) -> Result<()> {
    let node = NODE.get();

    let dlc_channels = node
        .inner
        .sub_channel_manager
        .get_dlc_manager()
        .get_store()
        .get_sub_channels()?;

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
