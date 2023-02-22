use crate::event;
use crate::event::EventInternal;
use crate::trade::order::OrderStateTrade;
use crate::trade::order::OrderTrade;
use crate::trade::order::OrderTypeTrade;
use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use anyhow::Context;
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub async fn submit_order(order: OrderTrade) -> Result<()> {
    // TODO: Save in DB and pass on to orderbook
    tokio::time::sleep(Duration::from_secs(5)).await;

    // TODO: The conversion to the API order should not be done in here, but should be done by
    // FlutterSubscriber! This requires a proper internal Even model. We should distinguish
    // between the internal events and those exposed on the API!
    event::publish(&EventInternal::OrderUpdateNotification(order));

    Ok(())
}

pub async fn get_order(id: String) -> Result<OrderTrade> {
    // TODO: Fetch from database

    let id = Uuid::from_str(id.as_str()).context("Failed to parse UUID")?;

    let dummy_order = OrderTrade {
        id,
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbolTrade::BtcUsd,
        direction: DirectionTrade::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStateTrade::Filled {
            execution_price: 25000.0,
        },
    };

    Ok(dummy_order)
}

pub async fn get_orders() -> Result<Vec<OrderTrade>> {
    // TODO: Fetch from database

    let dummy_order = OrderTrade {
        id: Uuid::new_v4(),
        leverage: 2.0,
        quantity: 1000.0,
        contract_symbol: ContractSymbolTrade::BtcUsd,
        direction: DirectionTrade::Long,
        order_type: OrderTypeTrade::Market,
        status: OrderStateTrade::Filled {
            execution_price: 25000.0,
        },
    };

    Ok(vec![dummy_order])
}
