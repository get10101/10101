use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order::OrderStateTrade;
use crate::trade::order::OrderTrade;
use crate::trade::order::OrderTypeTrade;
use crate::trade::position;
use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use anyhow::Context;
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;
use trade::ContractSymbol;
use trade::TradeParams;
use uuid::Uuid;

pub async fn submit_order(order: OrderTrade) -> Result<()> {
    // TODO: Save in DB and pass on to orderbook
    tokio::time::sleep(Duration::from_secs(5)).await;

    event::publish(&EventInternal::OrderUpdateNotification(order));

    // TODO: remove this and use the orderbook event to trigger the trade!
    let dummy_trade_params = TradeParams {
        pubkey: ln_dlc::get_node_info()?.pubkey,
        // We set this to our pubkey as well for simplicity until we receive a match from the
        // orderbook
        pubkey_counterparty: ln_dlc::get_node_info()?.pubkey,
        // We set this to our order as well for simplicity until we receive a match from the
        // orderbook
        order_id: "the orderbook will know".to_string(),
        order_id_counterparty: "the orderbook will know".to_string(),
        contract_symbol: ContractSymbol::BtcUsd,
        leverage: order.leverage,
        leverage_counterparty: 2.0,
        quantity: order.quantity,
        execution_price: 55_000.0,
        // in 24h
        expiry: Duration::from_secs(60 * 60 * 24),
        oracle_pk: ln_dlc::get_oracle_pubkey()?,
    };

    position::handler::trade(dummy_trade_params).await?;

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
