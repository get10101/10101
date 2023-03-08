use crate::calculations;
use crate::config;
use crate::config::api::Config;
use crate::db;
use crate::db::models::LastLogin;
use crate::event;
use crate::event::api::FlutterSubscriber;
use crate::ln_dlc;
use crate::logger;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use crate::trade::order::api::Order;
use crate::trade::position;
use crate::trade::position::api::Position;
use anyhow::Result;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;
pub use trade::ContractSymbol;
pub use trade::Direction;

/// Initialise logging infrastructure for Rust
pub fn init_logging(sink: StreamSink<logger::LogEntry>) {
    logger::create_log_stream(sink)
}

#[derive(Clone, Debug, Default)]
pub struct WalletInfo {
    pub balances: Balances,
    pub history: Vec<Transaction>,
}

#[derive(Clone, Debug, Default)]
pub struct Balances {
    pub on_chain: u64,
    pub lightning: u64,
}

pub fn refresh_wallet_info() -> Result<WalletInfo> {
    ln_dlc::get_wallet_info()
}

#[derive(Clone, Debug, Default)]
pub struct Transaction {
    // TODO(Restioson): newtype?
    pub address: String,
    pub flow: PaymentFlow,
    // TODO(Restioson): newtype?
    pub amount_sats: u64,
    pub wallet_type: WalletType,
}

#[derive(Clone, Debug, Default)]
pub enum WalletType {
    OnChain,
    #[default]
    Lightning,
}

#[derive(Clone, Debug, Default)]
pub enum PaymentFlow {
    #[default]
    Inbound,
    Outbound,
}

pub fn calculate_margin(price: f64, quantity: f64, leverage: f64) -> SyncReturn<u64> {
    SyncReturn(calculations::calculate_margin(price, quantity, leverage))
}

pub fn calculate_quantity(price: f64, margin: u64, leverage: f64) -> SyncReturn<f64> {
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
    price: f64,
    leverage: f64,
    direction: Direction,
) -> SyncReturn<f64> {
    SyncReturn(calculations::calculate_liquidation_price(
        price, leverage, direction,
    ))
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_order(order: NewOrder) -> Result<()> {
    order::handler::submit_order(order.into()).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_order(id: String) -> Result<Order> {
    let order = order::handler::get_order(id).await?.into();
    Ok(order)
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_orders() -> Result<Vec<Order>> {
    let orders = order::handler::get_orders()
        .await?
        .into_iter()
        .map(|order| order.into())
        .collect::<Vec<Order>>();

    Ok(orders)
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_positions() -> Result<Vec<Position>> {
    let positions = position::handler::get_positions()
        .await?
        .into_iter()
        .map(|order| order.into())
        .collect::<Vec<Position>>();

    Ok(positions)
}

pub fn subscribe(stream: StreamSink<event::api::Event>) {
    tracing::debug!("Subscribing flutter to event hub");
    event::subscribe(FlutterSubscriber::new(stream))
}

pub fn run(config: Config, app_dir: String) -> Result<()> {
    config::set(config);
    db::init_db(app_dir.clone())?;
    ln_dlc::run(app_dir)
}

pub fn get_new_address() -> SyncReturn<String> {
    SyncReturn(ln_dlc::get_new_address().unwrap())
}

pub fn open_channel() -> Result<()> {
    ln_dlc::open_channel()
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

pub fn update_last_login() -> Result<LastLogin> {
    let last_login = db::update_last_login()?;
    Ok(last_login)
}
