use crate::calculations;
use crate::ln_dlc;
use crate::ln_dlc::Balance;
use crate::logger;
use crate::model;
use crate::model::order::NewOrder;
use crate::model::order::Order;
use crate::model::order::OrderNotification;
use crate::model::Direction;
use crate::trade::order;
use anyhow::Result;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;

/// Initialise logging infrastructure for Rust
pub fn init_logging(sink: StreamSink<logger::LogEntry>) {
    logger::create_log_stream(sink)
}

pub fn calculate_margin(price: f64, quantity: f64, leverage: f64) -> SyncReturn<u64> {
    SyncReturn(calculations::calculate_margin(price, quantity, leverage))
}

pub fn calculate_quantity(price: f64, margin: u64, leverage: f64) -> SyncReturn<f64> {
    SyncReturn(calculations::calculate_quantity(price, margin, leverage))
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

#[allow(dead_code)]
#[derive(Clone)]
pub enum Event {
    Init(String),
    Log(String),
    OrderUpdateNotification(String),
    WalletInfo(Balance),
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_order(order: NewOrder) -> Result<()> {
    order::handler::submit_order(order.into()).await?;
    Ok(())
}

pub fn subscribe_to_order_notifications(sink: StreamSink<OrderNotification>) -> Result<()> {
    model::order::notifications::add_listener(sink)
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

pub fn run(stream: StreamSink<Event>, app_dir: String) -> Result<()> {
    ln_dlc::run(stream, app_dir)
}

pub fn get_new_address() -> SyncReturn<String> {
    SyncReturn(ln_dlc::get_new_address().unwrap())
}

pub fn open_channel() -> Result<()> {
    ln_dlc::open_channel()
}

pub fn create_invoice() -> Result<String> {
    Ok(ln_dlc::create_invoice()?.to_string())
}

pub fn send_payment(invoice: String) -> Result<()> {
    ln_dlc::send_payment(&invoice)
}
