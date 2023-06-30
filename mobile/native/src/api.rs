use crate::calculations;
use crate::channel_fee::ChannelFeePaymentSubscriber;
use crate::commons::api::Price;
use crate::config;
use crate::config::api::Config;
use crate::config::get_network;
use crate::db;
use crate::event;
use crate::event::api::FlutterSubscriber;
use crate::ln_dlc;
use crate::logger;
use crate::orderbook;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use crate::trade::order::api::Order;
use crate::trade::position;
use crate::trade::position::api::Position;
use anyhow::Context;
use anyhow::Result;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;
use lightning_invoice::Invoice;
use lightning_invoice::InvoiceDescription;
use std::backtrace::Backtrace;
use std::ops::Add;
use std::str::FromStr;
use std::time::Duration;
use std::time::SystemTime;
pub use trade::ContractSymbol;
pub use trade::Direction;

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
    pub wallet_type: WalletType,
}

#[derive(Clone, Debug)]
pub enum WalletType {
    OnChain { txid: String },
    Lightning { payment_hash: String },
    Trade { order_id: String },
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

/// Wrapper for Flutter purposes, it ensures we do not return a Result, which
/// would have otherwise be converted into an exception.
/// There is no recovery possible, and panicking is the only option ensuring
/// that we will get an adequate crash report.
pub fn run_in_flutter(config: Config, app_dir: String, seed_dir: String) {
    run(config, app_dir, seed_dir).expect("Failed to start the backend");
}

pub fn run(config: Config, app_dir: String, seed_dir: String) -> Result<()> {
    std::panic::set_hook(
        #[allow(clippy::print_stderr)]
        Box::new(|info| {
            let backtrace = Backtrace::force_capture();

            tracing::error!(%info, "Aborting after panic in task");
            eprintln!("{backtrace}");

            std::process::abort()
        }),
    );

    config::set(config);
    db::init_db(&app_dir, get_network())?;
    let runtime = ln_dlc::get_or_create_tokio_runtime()?;
    ln_dlc::run(app_dir, seed_dir, runtime)?;
    event::subscribe(ChannelFeePaymentSubscriber::new());
    orderbook::subscribe(ln_dlc::get_node_key(), runtime)
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
    order::register_beta(email).await
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

pub fn get_node_id() -> SyncReturn<String> {
    SyncReturn(ln_dlc::get_node_info().pubkey.to_string())
}
