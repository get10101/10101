use crate::calculations::calculate_liquidation_price;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order;
use crate::trade::order::Order;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use anyhow::Context;
use anyhow::Result;
use coordinator_commons::TradeParams;
use orderbook_commons::FilledWith;
use rust_decimal::prelude::ToPrimitive;
use trade::ContractSymbol;
use trade::Direction;

/// Sets up a trade with the counterparty
///
/// In a success scenario this results in creating, updating or deleting a position.
/// The DLC that represents the position will be stored in the database.
/// Errors are handled within the scope of this function.
pub async fn trade(filled: FilledWith) -> Result<()> {
    let order = db::get_order(filled.order_id).context("Could not load order from db")?;

    let trade_params = TradeParams {
        pubkey: ln_dlc::get_node_info()?.pubkey,
        contract_symbol: ContractSymbol::BtcUsd,
        leverage: order.leverage,
        quantity: order.quantity,
        direction: Direction::Long,
        filled_with: filled,
    };

    let execution_price = trade_params
        .weighted_execution_price()
        .to_f64()
        .expect("to fit into f64");
    order::handler::order_filling(order.id, execution_price)
        .context("Could not update order to filling")?;

    if let Err((reason, e)) = ln_dlc::trade(trade_params).await {
        order::handler::order_failed(Some(order.id), reason, e)
            .context("Could not set order to failed")?;
    }

    Ok(())
}

/// Fetch the positions from the database
pub async fn get_positions() -> Result<Vec<Position>> {
    db::get_positions()
}

/// Update position once an order was filled
///
/// This crates or updates the position.
/// If the position was closed we set it to `Closed` state.
pub fn position_update(filled_order: Order, collateral: u64) -> Result<()> {
    // We don't have a position yet
    if db::get_positions()?.is_empty() {
        tracing::debug!("We don't have a position at the moment, creating it");

        let average_entry_price = filled_order.execution_price().unwrap_or(0.0);
        let have_a_position = Position {
            leverage: filled_order.leverage,
            quantity: filled_order.quantity,
            contract_symbol: filled_order.contract_symbol,
            direction: filled_order.direction,
            average_entry_price,
            // TODO: Is it correct to use the average entry price to calculate the liquidation
            // price? -> What would that mean in the UI if we already have a
            // position and trade?
            liquidation_price: calculate_liquidation_price(
                average_entry_price,
                filled_order.leverage,
                filled_order.direction,
            ),
            // TODO: Remove the PnL, that has to be calculated in the UI
            position_state: PositionState::Open,
            collateral,
        };

        let position = db::insert_position(have_a_position)?;
        event::publish(&EventInternal::PositionUpdateNotification(position));
    } else {
        db::delete_positions()?;

        event::publish(&EventInternal::PositionCloseNotification(
            ContractSymbol::BtcUsd,
        ));
    }

    Ok(())
}
