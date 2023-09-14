use crate::calculations;
use crate::commons::api::ChannelInfo;
use crate::commons::api::Price;
use crate::config;
use crate::config::api::Config;
use crate::config::get_network;
use crate::db;
use crate::event;
use crate::event::api::FlutterSubscriber;
use crate::health;
use crate::ln_dlc;
use crate::ln_dlc::FUNDING_TX_WEIGHT_ESTIMATE;
use crate::logger;
use crate::orderbook;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use crate::trade::order::api::Order;
use crate::trade::position;
use crate::trade::position::api::Position;
use crate::trade::users;
use anyhow::Context;
use anyhow::Result;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;
use lightning_invoice::Invoice;
use lightning_invoice::InvoiceDescription;
use orderbook_commons::order_matching_fee_taker;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use state::Storage;
use std::backtrace::Backtrace;
use std::ops::Add;
use std::str::FromStr;
use std::time::Duration;
use std::time::SystemTime;
pub use trade::ContractSymbol;
pub use trade::Direction;

/// Allows the hot restart to work
static IS_INITIALISED: Storage<bool> = Storage::new();

/// Initialise logging infrastructure for Rust
pub fn init_logging(sink: StreamSink<logger::LogEntry>) {
    logger::create_log_stream(sink)
}

#[derive(Clone, Debug, Default)]
pub struct WalletInfo {
    pub balances: Balances,
    pub history: Vec<WalletHistoryItem>,
}

#[derive(Clone, Debug, Default)]
pub struct Balances {
    pub on_chain: u64,
    pub lightning: u64,
}

/// Assembles the wallet info and publishes wallet info update event
#[tokio::main(flavor = "current_thread")]
pub async fn refresh_wallet_info() -> Result<()> {
    ln_dlc::refresh_wallet_info().await?;

    Ok(())
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
    },
    Trade {
        order_id: String,
    },
    OrderMatchingFee {
        order_id: String,
        payment_hash: String,
    },
    JitChannelFee {
        funding_txid: String,
        payment_hash: String,
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

pub fn subscribe(stream: StreamSink<event::api::Event>) {
    tracing::debug!("Subscribing flutter to event hub");
    event::subscribe(FlutterSubscriber::new(stream))
}

/// Wrapper for Flutter purposes - can throw an exception.
pub fn run_in_flutter(config: Config, app_dir: String, seed_dir: String) -> Result<()> {
    let result = if !IS_INITIALISED.try_get().unwrap_or(&false) {
        run(config, app_dir, seed_dir, IncludeBacktraceOnPanic::Yes)
            .context("Failed to start the backend")
    } else {
        Ok(())
    };
    IS_INITIALISED.set(true);
    result
}

#[derive(PartialEq)]
pub enum IncludeBacktraceOnPanic {
    Yes,
    No,
}

pub fn run(
    config: Config,
    app_dir: String,
    seed_dir: String,
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

    config::set(config.clone());
    db::init_db(&app_dir, get_network())?;
    let runtime = ln_dlc::get_or_create_tokio_runtime()?;
    ln_dlc::run(app_dir, seed_dir, runtime)?;

    let (_health, tx) = health::Health::new(config, runtime);

    orderbook::subscribe(ln_dlc::get_node_key(), runtime, tx.orderbook)
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
    Ok(channel_details.map(|channel_details| channel_details.into()))
}

pub fn coordinator_liquidity_multiplier() -> SyncReturn<u64> {
    SyncReturn(ln_dlc_node::LIQUIDITY_MULTIPLIER)
}

pub fn max_channel_value() -> Result<u64> {
    ln_dlc::max_channel_value().map(|amount| amount.to_sat())
}

pub fn contract_tx_fee_rate() -> Result<u64> {
    ln_dlc::contract_tx_fee_rate()
}

pub fn create_invoice_with_amount(amount_sats: u64) -> Result<String> {
    Ok(ln_dlc::create_invoice(Some(amount_sats))?.to_string())
}

pub fn create_invoice_without_amount() -> Result<String> {
    Ok(ln_dlc::create_invoice(None)?.to_string())
}

pub fn send_payment(invoice: String) -> Result<()> {
    ln_dlc::send_payment(&invoice)
}

pub struct LastLogin {
    pub id: i32,
    pub date: String,
}

pub fn update_last_login() -> Result<LastLogin> {
    let last_login = db::update_last_login()?;
    Ok(last_login)
}

pub fn get_seed_phrase() -> SyncReturn<Vec<String>> {
    SyncReturn(ln_dlc::get_seed_phrase())
}

/// Enroll a user in the beta program
#[tokio::main(flavor = "current_thread")]
pub async fn register_beta(email: String) -> Result<()> {
    users::register_beta(email).await
}

/// Send the Firebase token to the LSP for push notifications
#[tokio::main(flavor = "current_thread")]
pub async fn update_fcm_token(fcm_token: String) -> Result<()> {
    users::update_fcm_token(fcm_token).await
}

pub struct LightningInvoice {
    pub description: String,
    pub amount_sats: u64,
    pub timestamp: u64,
    pub payee: String,
    pub expiry: u64,
}

pub fn decode_invoice(invoice: String) -> Result<LightningInvoice> {
    anyhow::ensure!(!invoice.is_empty(), "received empty invoice");
    let invoice = &Invoice::from_str(&invoice).context("Could not parse invoice string")?;
    let description = match invoice.description() {
        InvoiceDescription::Direct(direct) => direct.to_string(),
        InvoiceDescription::Hash(_) => "".to_string(),
    };

    let timestamp = invoice.timestamp();

    let expiry = timestamp
        .add(Duration::from_secs(invoice.expiry_time().as_secs()))
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let timestamp = timestamp.duration_since(SystemTime::UNIX_EPOCH)?.as_secs();

    let payee = match invoice.payee_pub_key() {
        Some(pubkey) => pubkey.to_string(),
        None => invoice.recover_payee_pub_key().to_string(),
    };

    let amount_sats = (invoice.amount_milli_satoshis().unwrap_or(0) as f64 / 1000.0) as u64;

    Ok(LightningInvoice {
        description,
        timestamp,
        expiry,
        amount_sats,
        payee,
    })
}

pub fn get_node_id() -> Result<SyncReturn<String>> {
    Ok(SyncReturn(ln_dlc::get_node_info()?.pubkey.to_string()))
}

pub fn get_channel_open_fee_estimate_sat() -> Result<u64> {
    let fee_rate = ln_dlc::get_fee_rate()?;
    let estimate = FUNDING_TX_WEIGHT_ESTIMATE as f32 * fee_rate.as_sat_per_vb();

    Ok(estimate.ceil() as u64)
}
