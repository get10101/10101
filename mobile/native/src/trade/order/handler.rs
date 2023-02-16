use crate::api_model::order::notifications::send_notification;
use crate::api_model::order::OrderNotification;
use crate::api_model::order::OrderNotificationType;
use crate::api_model::order::OrderStatus;
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
    // TODO: safe order in db,

    match order.order_type {
        OrderTypeTrade::Market => {

            // TODO: submit order to orderbook

            // TODO: response success: returns the orderbook_id
            //      1. update order in database as success + set the orderbook_id
            // Note: The orderbook will notify us with updates on the order (handled in a separate
            // global stream)

            // TODO: response failure
            //      1. update the order in the database as failure
            //      1. early return failure (note: we don't have to safe it then and fail "early"
            // for now)
        }
        OrderTypeTrade::Limit { .. } => unimplemented!(),
    }

    // TODO: Remove this once the above TODOs are actions
    tokio::time::sleep(Duration::from_secs(10)).await;

    send_notification(OrderNotification {
        id: order.id.to_string(),
        notification_type: OrderNotificationType::New,
    });

    Ok(())
}

pub async fn update_order_status(id: Uuid, _status: OrderStatus) -> Result<()> {
    // TODO: Update order status in database

    send_notification(OrderNotification {
        id: id.to_string(),
        notification_type: OrderNotificationType::Update,
    });

    Ok(())
}

pub async fn get_order(id: String) -> Result<OrderTrade> {
    // TODO: Fetch from database

    let id = Uuid::from_str(id.as_str()).context("Failed to parse UUID")?;

    let dummy_order = OrderTrade {
        id,
        orderbook_id: Some(Uuid::new_v4()),
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStatusTrade::Filled,
    };

    Ok(dummy_order)
}

pub async fn get_open_and_filled_orders() -> Result<Vec<OrderTrade>> {
    // TODO: Fetch from database excluding orders with status "initial" and "failed"

    let dummy_order = OrderTrade {
        id: Uuid::new_v4(),
        orderbook_id: Some(Uuid::new_v4()),
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStatusTrade::Filled,
    };

    Ok(vec![dummy_order])
}
