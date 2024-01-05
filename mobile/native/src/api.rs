use crate::calculations;
use crate::commons::api::ChannelInfo;
use crate::commons::api::Price;
use crate::config;
use crate::config::api::Config;
use crate::config::api::Directories;
use crate::config::get_network;
use crate::db;
use crate::destination;
use crate::event;
use crate::event::api::FlutterSubscriber;
use crate::health;
use crate::ln_dlc;
use crate::ln_dlc::get_storage;
use crate::ln_dlc::FUNDING_TX_WEIGHT_ESTIMATE;
use crate::logger;
use crate::orderbook;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use crate::trade::order::api::Order;
use crate::trade::position;
use crate::trade::position::api::Position;
use crate::trade::users;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use commons::order_matching_fee_taker;
use commons::OrderbookRequest;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;
use lightning::chain::chaininterface::ConfirmationTarget as LnConfirmationTarget;
use lightning::util::persist::KVStore;
use lightning::util::persist::NETWORK_GRAPH_PERSISTENCE_KEY;
use lightning::util::persist::NETWORK_GRAPH_PERSISTENCE_PRIMARY_NAMESPACE;
use lightning::util::persist::NETWORK_GRAPH_PERSISTENCE_SECONDARY_NAMESPACE;
use ln_dlc_node::channel::UserChannelId;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::backtrace::Backtrace;
use std::path::PathBuf;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::channel;
pub use trade::ContractSymbol;
pub use trade::Direction;

/// Initialise logging infrastructure for Rust
pub fn init_logging(sink: StreamSink<logger::LogEntry>) {
    logger::create_log_stream(sink)
}

#[derive(Clone, Debug, Default)]
pub struct LspConfig {
    pub contract_tx_fee_rate: u64,
    pub liquidity_options: Vec<LiquidityOption>,
}

impl From<commons::LspConfig> for LspConfig {
    fn from(value: commons::LspConfig) -> Self {
        Self {
            contract_tx_fee_rate: value.contract_tx_fee_rate,
            liquidity_options: value
                .liquidity_options
                .into_iter()
                .map(|lo| lo.into())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WalletInfo {
    pub balances: Balances,
    pub history: Vec<WalletHistoryItem>,
}

#[derive(Clone, Debug, Default)]
pub struct Balances {
    pub on_chain: u64,
    pub off_chain: u64,
}

/// Assembles the wallet info and publishes wallet info update event.
#[tokio::main(flavor = "current_thread")]
pub async fn refresh_wallet_info() -> Result<()> {
    ln_dlc::refresh_wallet_info().await?;

    Ok(())
}

pub fn refresh_lightning_wallet() -> Result<()> {
    ln_dlc::refresh_lightning_wallet()
}

#[derive(Clone, Debug)]
pub struct WalletHistoryItem {
    pub flow: PaymentFlow,
    pub amount_sats: u64,
    pub timestamp: u64,
    pub status: Status,
    pub wallet_type: WalletHistoryItemType,
}

#[derive(Clone, Debug)]
pub enum WalletHistoryItemType {
    OnChain {
        txid: String,
        fee_sats: Option<u64>,
        confirmations: u64,
    },
    Lightning {
        payment_hash: String,
        description: String,
        payment_preimage: Option<String>,
        invoice: Option<String>,
        fee_msat: Option<u64>,
        expiry_timestamp: Option<u64>,
        funding_txid: Option<String>,
    },
    Trade {
        order_id: String,
        fee_sat: u64,
        pnl: Option<i64>,
        contracts: u64,
        direction: String,
    },
}

#[derive(Clone, Debug, Default)]
pub enum PaymentFlow {
    #[default]
    Inbound,
    Outbound,
}

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Pending,
    Confirmed,
    Expired,
    Failed,
}

pub fn calculate_margin(price: f32, quantity: f32, leverage: f32) -> SyncReturn<u64> {
    SyncReturn(calculations::calculate_margin(price, quantity, leverage))
}

pub fn calculate_quantity(price: f32, margin: u64, leverage: f32) -> SyncReturn<f32> {
    SyncReturn(calculations::calculate_quantity(price, margin, leverage))
}

#[allow(dead_code)]
#[frb(mirror(ContractSymbol))]
#[derive(Debug, Clone, Copy)]
pub enum _ContractSymbol {
    BtcUsd,
}

#[allow(dead_code)]
#[frb(mirror(Direction))]
#[derive(Debug, Clone, Copy)]
pub enum _Direction {
    Long,
    Short,
}

pub fn calculate_liquidation_price(
    price: f32,
    leverage: f32,
    direction: Direction,
) -> SyncReturn<f32> {
    SyncReturn(calculations::calculate_liquidation_price(
        price, leverage, direction,
    ))
}

pub fn calculate_pnl(
    opening_price: f32,
    closing_price: Price,
    quantity: f32,
    leverage: f32,
    direction: Direction,
) -> SyncReturn<i64> {
    // TODO: Handle the result and don't just return 0

    SyncReturn(
        calculations::calculate_pnl(
            opening_price,
            closing_price.into(),
            quantity,
            leverage,
            direction,
        )
        .unwrap_or(0),
    )
}

/// Calculate the order matching fee that the app user will have to pay for if the corresponding
/// trade gets executed.
///
/// This is only an estimate as the price may change slightly. Also, the coordinator could choose to
/// change the fee structure independently.
pub fn order_matching_fee(quantity: f32, price: f32) -> SyncReturn<u64> {
    let price = Decimal::from_f32(price).expect("price to fit in Decimal");

    let order_matching_fee = order_matching_fee_taker(quantity, price).to_sat();

    SyncReturn(order_matching_fee)
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_order(order: NewOrder) -> Result<String> {
    order::handler::submit_order(order.into())
        .await
        .map(|id| id.to_string())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_orders() -> Result<Vec<Order>> {
    let orders = order::handler::get_orders_for_ui()
        .await?
        .into_iter()
        .map(|order| order.into())
        .collect::<Vec<Order>>();

    Ok(orders)
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_async_order() -> Result<Option<Order>> {
    let order = order::handler::get_async_order()?;
    let order = order.map(|order| order.into());
    Ok(order)
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_positions() -> Result<Vec<Position>> {
    let positions = position::handler::get_positions()?
        .into_iter()
        .map(|order| order.into())
        .collect::<Vec<Position>>();

    Ok(positions)
}

pub fn delete_network_graph() -> Result<()> {
    crate::state::get_storage()
        .ln_storage
        .remove(
            NETWORK_GRAPH_PERSISTENCE_PRIMARY_NAMESPACE,
            NETWORK_GRAPH_PERSISTENCE_SECONDARY_NAMESPACE,
            NETWORK_GRAPH_PERSISTENCE_KEY,
            false,
        )
        .map_err(|e| anyhow!("{e:#}"))
}

pub fn subscribe(stream: StreamSink<event::api::Event>) {
    tracing::debug!("Subscribing flutter to event hub");
    event::subscribe(FlutterSubscriber::new(stream))
}

/// Wrapper for Flutter purposes - can throw an exception.
pub fn run_in_flutter(seed_dir: String, fcm_token: String) -> Result<()> {
    match crate::state::try_get_websocket() {
        None => {
            let (tx_websocket, _rx) = channel::<OrderbookRequest>(10);
            run_internal(
                seed_dir,
                fcm_token,
                tx_websocket.clone(),
                IncludeBacktraceOnPanic::Yes,
            )
            .context("Failed to start the backend")?;

            crate::state::set_websocket(tx_websocket);
        }
        Some(tx_websocket) => {
            // In case of a hot-restart we do not start the node again as it is already running.
            // However, we need to re-send the authentication message to get the initial data from
            // the coordinator and trigger a new user login event.
            tracing::info!("Re-sending authentication message");

            let signature =
                orderbook_client::create_auth_message_signature(move |msg| commons::Signature {
                    pubkey: ln_dlc::get_node_pubkey(),
                    signature: ln_dlc::get_node_key().sign_ecdsa(msg),
                });

            let runtime = crate::state::get_or_create_tokio_runtime()?;
            runtime.block_on(async {
                tx_websocket.send(OrderbookRequest::Authenticate {
                    fcm_token: Some(fcm_token),
                    signature,
                })
            })?;
        }
    };

    Ok(())
}

/// Wrapper for the tests.
///
/// Needed as we do not want to have a hot restart handling in the tests and also can't expose a
/// channel::Sender through frb.
pub fn run_in_test(seed_dir: String) -> Result<()> {
    let (tx_websocket, _rx) = channel::<OrderbookRequest>(10);
    run_internal(
        seed_dir,
        "".to_string(),
        tx_websocket,
        IncludeBacktraceOnPanic::No,
    )
}

#[derive(PartialEq)]
pub enum IncludeBacktraceOnPanic {
    Yes,
    No,
}

pub fn set_config(config: Config, app_dir: String, seed_dir: String) -> Result<()> {
    crate::state::set_config((config, Directories { app_dir, seed_dir }).into());
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn full_backup() -> Result<()> {
    db::init_db(&config::get_data_dir(), get_network())?;
    get_storage().full_backup().await
}

fn run_internal(
    seed_dir: String,
    fcm_token: String,
    tx_websocket: broadcast::Sender<OrderbookRequest>,
    backtrace_on_panic: IncludeBacktraceOnPanic,
) -> Result<()> {
    if backtrace_on_panic == IncludeBacktraceOnPanic::Yes {
        std::panic::set_hook(
            #[allow(clippy::print_stderr)]
            Box::new(|info| {
                let backtrace = Backtrace::force_capture();

                tracing::error!(%info, "Aborting after panic in task");
                eprintln!("{backtrace}");

                std::process::abort()
            }),
        );
    }

    db::init_db(&config::get_data_dir(), get_network())?;

    let runtime = crate::state::get_or_create_tokio_runtime()?;
    ln_dlc::run(seed_dir, runtime)?;

    let (_health, tx) = health::Health::new(runtime);

    orderbook::subscribe(
        ln_dlc::get_node_key(),
        runtime,
        tx.orderbook,
        fcm_token,
        tx_websocket,
    )
}

pub fn get_unused_address() -> SyncReturn<String> {
    SyncReturn(ln_dlc::get_unused_address())
}

pub fn close_channel() -> Result<()> {
    ln_dlc::close_channel(false)
}

pub fn force_close_channel() -> Result<()> {
    ln_dlc::close_channel(true)
}

/// Returns channel info if we have a channel available already
///
/// If no channel is established with the coordinator `None` is returned.
pub fn channel_info() -> Result<Option<ChannelInfo>> {
    let channel_details = ln_dlc::get_usable_channel_details()?.first().cloned();
    let channel_info = match channel_details {
        Some(channel_details) => {
            let user_channel_id = UserChannelId::from(channel_details.user_channel_id);
            let channel = db::get_channel(&user_channel_id.to_string())?
                .with_context(|| format!("Couldn't find channel for {user_channel_id}!"))?;

            Some(ChannelInfo {
                channel_capacity: channel_details.channel_value_satoshis,
                reserve: channel_details.unspendable_punishment_reserve,
                liquidity_option_id: channel.liquidity_option_id,
            })
        }
        None => None,
    };
    Ok(channel_info)
}

pub fn max_channel_value() -> Result<u64> {
    ln_dlc::max_channel_value().map(|amount| amount.to_sat())
}

pub fn contract_tx_fee_rate() -> Result<Option<u64>> {
    ln_dlc::contract_tx_fee_rate()
}

#[derive(Debug, Clone)]
pub struct LiquidityOption {
    pub id: i32,
    pub rank: usize,
    pub title: String,
    /// the amount the trader can trade up to
    pub trade_up_to_sats: u64,
    /// min deposit in sats
    pub min_deposit_sats: u64,
    /// max deposit in sats
    pub max_deposit_sats: u64,
    /// min fee in sats
    pub min_fee_sats: u64,
    pub fee_percentage: f64,
    pub coordinator_leverage: f32,
    pub active: bool,
}

impl From<commons::LiquidityOption> for LiquidityOption {
    fn from(value: commons::LiquidityOption) -> Self {
        LiquidityOption {
            id: value.id,
            rank: value.rank,
            title: value.title,
            trade_up_to_sats: value.trade_up_to_sats,
            min_deposit_sats: value.min_deposit_sats,
            max_deposit_sats: value.max_deposit_sats,
            min_fee_sats: value.min_fee_sats,
            fee_percentage: value.fee_percentage,
            coordinator_leverage: value.coordinator_leverage,
            active: value.active,
        }
    }
}

pub fn create_onboarding_invoice(
    liquidity_option_id: i32,
    amount_sats: u64,
    fee_sats: u64,
) -> Result<String> {
    Ok(ln_dlc::create_onboarding_invoice(liquidity_option_id, amount_sats, fee_sats)?.to_string())
}

pub struct PaymentRequest {
    pub bip21: String,
    pub lightning: String,
}

pub fn create_payment_request(
    amount_sats: Option<u64>,
    _is_usdp: bool,
    _description: String,
) -> Result<PaymentRequest> {
    let amount_query = amount_sats
        .map(|amt| format!("?amount={}", Amount::from_sat(amt).to_btc()))
        .unwrap_or_default();
    let addr = ln_dlc::get_unused_address();

    // TODO(bonomat): this can be removed as we don't support LN for now
    // let ln_invoice = if is_usdp {
    //     ln_dlc::create_usdp_invoice(amount_sats, description)?
    // } else {
    //     ln_dlc::create_invoice(amount_sats, description)?
    // };

    Ok(PaymentRequest {
        bip21: format!("bitcoin:{addr}{amount_query}"),
        lightning: "TODO: remove this part".to_string(),
    })
}

pub fn is_usdp_payment(payment_hash: String) -> SyncReturn<bool> {
    SyncReturn(ln_dlc::is_usdp_payment(payment_hash))
}

pub enum SendPayment {
    Lightning {
        invoice: String,
        amount: Option<u64>,
    },
    OnChain {
        address: String,
        amount: u64,
        fee: Fee,
    },
}

/// The choice of on-chain network fee
pub enum Fee {
    /// A fee based on the priority of the payment
    Priority(ConfirmationTarget),
    /// A custom fee
    Custom { sats: u64 },
}

impl From<Fee> for ln_dlc_node::node::Fee {
    fn from(value: Fee) -> Self {
        match value {
            Fee::Priority(target) => ln_dlc_node::node::Fee::Priority(target.into()),
            Fee::Custom { sats } => ln_dlc_node::node::Fee::Custom(Amount::from_sat(sats)),
        }
    }
}

/// Analogous to [`lightning::chain::chaininterface::ConfirmationTarget`] but for the Flutter API
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ConfirmationTarget {
    Minimum,
    Background,
    Normal,
    HighPriority,
}

impl From<ConfirmationTarget> for LnConfirmationTarget {
    fn from(value: ConfirmationTarget) -> Self {
        match value {
            ConfirmationTarget::Minimum => LnConfirmationTarget::MempoolMinimum,
            ConfirmationTarget::Background => LnConfirmationTarget::Background,
            ConfirmationTarget::Normal => LnConfirmationTarget::Normal,
            ConfirmationTarget::HighPriority => LnConfirmationTarget::HighPriority,
        }
    }
}

pub struct FeeEstimation {
    pub sats_per_vbyte: u64,
    pub total_sats: u64,
}

/// Calculate the fees for an on-chain transaction, using the 3 default fee rates (background,
/// normal, and high priority). This both estimates the fee rate and calculates the TX size to get
/// the overall fee for a given TX.
pub fn calculate_all_fees_for_on_chain(address: String, amount: u64) -> Result<Vec<FeeEstimation>> {
    const TARGETS: [ConfirmationTarget; 4] = [
        ConfirmationTarget::Minimum,
        ConfirmationTarget::Background,
        ConfirmationTarget::Normal,
        ConfirmationTarget::HighPriority,
    ];

    let runtime = crate::state::get_or_create_tokio_runtime()?;
    runtime.block_on(async {
        let mut fees = Vec::with_capacity(TARGETS.len());

        for confirmation_target in TARGETS {
            let payment = SendPayment::OnChain {
                address: address.clone(),
                amount,
                fee: Fee::Priority(confirmation_target),
            };

            fees.push(FeeEstimation {
                sats_per_vbyte: (fee_rate(confirmation_target)?.ceil()) as u64,
                total_sats: ln_dlc::estimate_payment_fee_msat(payment).await? / 1000,
            })
        }

        Ok(fees)
    })
}

pub fn fee_rate(confirmation_target: ConfirmationTarget) -> Result<f32> {
    ln_dlc::get_fee_rate_for_target(confirmation_target.into()).map(|rate| rate.as_sat_per_vb())
}

pub fn send_payment(payment: SendPayment) -> Result<()> {
    let runtime = crate::state::get_or_create_tokio_runtime()?;
    runtime.block_on(async { ln_dlc::send_payment(payment).await })
}

pub fn send_preflight_probe(payment: SendPayment) -> Result<u64> {
    let runtime = crate::state::get_or_create_tokio_runtime()?;
    runtime.block_on(async { ln_dlc::estimate_payment_fee_msat(payment).await })
}

pub struct LastLogin {
    pub id: i32,
    pub date: String,
}

pub fn get_seed_phrase() -> SyncReturn<Vec<String>> {
    SyncReturn(ln_dlc::get_seed_phrase())
}

#[tokio::main(flavor = "current_thread")]
pub async fn restore_from_seed_phrase(
    seed_phrase: String,
    target_seed_file_path: String,
) -> Result<()> {
    let file_path = PathBuf::from(target_seed_file_path);
    tracing::info!("Restoring seed from phrase to {:?}", file_path);
    ln_dlc::restore_from_mnemonic(&seed_phrase, file_path.as_path()).await?;
    Ok(())
}

pub fn init_new_mnemonic(target_seed_file_path: String) -> Result<()> {
    let file_path = PathBuf::from(target_seed_file_path);
    tracing::info!("Creating a new seed in {:?}", file_path);
    ln_dlc::init_new_mnemonic(file_path.as_path())
}

/// Enroll a user in the beta program
#[tokio::main(flavor = "current_thread")]
pub async fn register_beta(email: String) -> Result<()> {
    users::register_beta(email).await
}

pub enum Destination {
    Bolt11 {
        description: String,
        amount_sats: u64,
        timestamp: u64,
        payee: String,
        expiry: u64,
    },
    OnChainAddress(String),
    Bip21 {
        address: String,
        label: String,
        message: String,
        amount_sats: Option<u64>,
    },
}

pub fn decode_destination(destination: String) -> Result<Destination> {
    ensure!(!destination.is_empty(), "Destination must be set");
    destination::decode_destination(destination)
}

pub fn get_node_id() -> SyncReturn<String> {
    SyncReturn(ln_dlc::get_node_pubkey().to_string())
}

pub fn get_channel_open_fee_estimate_sat() -> Result<u64> {
    let fee_rate = ln_dlc::get_fee_rate()?;
    let estimate = FUNDING_TX_WEIGHT_ESTIMATE as f32 * fee_rate.as_sat_per_vb();

    Ok(estimate.ceil() as u64)
}

pub fn get_expiry_timestamp(network: String) -> SyncReturn<i64> {
    let network = config::api::parse_network(&network);
    SyncReturn(commons::calculate_next_expiry(OffsetDateTime::now_utc(), network).unix_timestamp())
}
