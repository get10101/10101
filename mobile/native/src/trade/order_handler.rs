use crate::common::ContractSymbol;
use crate::common::Direction;
use crate::trade::order::Order;
use crate::trade::order::OrderStatus;
use crate::trade::order::OrderTypeTrade;
use anyhow::Result;
use std::time::Duration;
use uuid::Uuid;

pub async fn process_new_order(_order: Order) -> Result<()> {
    // TODO: Save in DB and pass on to orderbook
    tokio::time::sleep(Duration::from_secs(10)).await;
    Ok(())
}

pub async fn get_orders() -> Vec<Order> {
    // TODO: Fetch from database

    let dummy_order = Order {
        id: Uuid::new_v4(),
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStatus::Filled,
    };

    vec![dummy_order]
}
