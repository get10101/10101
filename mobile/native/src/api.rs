use crate::calculations;
use crate::commons::api::Price;
use crate::config;
use crate::config::api::Config;
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
    pub history: Vec<WalletHistoryItem>,
}

#[derive(Clone, Debug, Default)]
pub struct Balances {
    pub on_chain: u64,
    pub lightning: u64,
}

/// Assembles the wallet info and publishes wallet info update event
pub fn refresh_wallet_info() -> Result<()> {
    ln_dlc::refresh_wallet_info()
}

#[derive(Clone, Debug, Default)]
pub struct WalletHistoryItem {
    pub flow: PaymentFlow,
    pub amount_sats: u64,
    pub timestamp: u64,
    pub status: Status,
    pub wallet_type: WalletType,
}

#[derive(Clone, Debug)]
pub enum WalletType {
    OnChain { address: String, txid: String },
    Lightning { counterparty_node_id: String },
    Trade { order_id: String },
}

impl Default for WalletType {
    fn default() -> Self {
        WalletType::Lightning {
            counterparty_node_id: "".to_string(),
        }
    }
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

pub fn calculate_pnl(
    opening_price: f64,
    closing_price: Price,
    quantity: f64,
    leverage: f64,
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
pub async fn submit_order(order: NewOrder) -> Result<()> {
    order::handler::submit_order(order.into()).await?;
    Ok(())
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
    ln_dlc::run(app_dir)?;
    orderbook::subscribe(ln_dlc::get_node_key()?)
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

pub struct LastLogin {
    pub id: i32,
    pub date: String,
}

pub fn update_last_login() -> Result<LastLogin> {
    let last_login = db::update_last_login()?;
    Ok(last_login)
}
