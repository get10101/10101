use crate::api_model::order::notifications::send_notification;
use crate::api_model::order::OrderNotification;
use crate::api_model::order::OrderNotificationType;
use crate::common::ContractSymbol;
use crate::common::Direction;
use crate::trade::order::OrderStatusTrade;
use crate::trade::order::OrderTrade;
use crate::trade::order::OrderTypeTrade;
use anyhow::Context;
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub async fn process_new_order(order: OrderTrade) -> Result<()> {
    // TODO: Save in DB and pass on to orderbook
    tokio::time::sleep(Duration::from_secs(10)).await;

    send_notification(OrderNotification {
        id: order.id.to_string(),
        notification_type: OrderNotificationType::New,
    });

    Ok(())
}

pub async fn get_order(id: String) -> Result<OrderTrade> {
    // TODO: Fetch from database

    let id = Uuid::from_str(id.as_str()).context("Failed to parse UUID")?;

    let dummy_order = OrderTrade {
        id,
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStatusTrade::Filled,
    };

    Ok(dummy_order)
}

pub async fn get_orders() -> Result<Vec<OrderTrade>> {
    // TODO: Fetch from database

    let dummy_order = OrderTrade {
        id: Uuid::new_v4(),
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStatusTrade::Filled,
    };

    Ok(vec![dummy_order])
}
