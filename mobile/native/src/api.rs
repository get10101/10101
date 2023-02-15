use crate::api_calculations;
use crate::api_model;
use crate::api_model::order::NewOrder;
use crate::api_model::order::Order;
use crate::api_model::order::OrderNotification;
use crate::common::Direction;
use crate::logger;
use crate::trade::order;
use anyhow::Result;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;

/// Initialise logging infrastructure for Rust
pub fn init_logging(sink: StreamSink<logger::LogEntry>) {
    logger::create_log_stream(sink)
}

pub fn calculate_margin(price: f64, quantity: f64, leverage: f64) -> SyncReturn<u64> {
    SyncReturn(api_calculations::calculate_margin(
        price, quantity, leverage,
    ))
}

pub fn calculate_quantity(price: f64, margin: u64, leverage: f64) -> SyncReturn<f64> {
    SyncReturn(api_calculations::calculate_quantity(
        price, margin, leverage,
    ))
}

pub fn calculate_liquidation_price(
    price: f64,
    leverage: f64,
    direction: Direction,
) -> SyncReturn<f64> {
    SyncReturn(api_calculations::calculate_liquidation_price(
        price, leverage, direction,
    ))
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum Event {
    Log(String),
    OrderUpdateNotification(String),
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_order(order: NewOrder) -> Result<()> {
    order::handler::submit_order(order.into()).await?;
    Ok(())
}

pub fn subscribe_to_order_notifications(sink: StreamSink<OrderNotification>) -> Result<()> {
    api_model::order::notifications::add_listener(sink)
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
