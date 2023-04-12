use crate::calculations::calculate_liquidation_price;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order;
use crate::trade::order::Order;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use coordinator_commons::TradeParams;
use orderbook_commons::FilledWith;
use orderbook_commons::Prices;
use rust_decimal::prelude::ToPrimitive;
use trade::ContractSymbol;

/// Sets up a trade with the counterparty
///
/// In a success scenario this results in creating, updating or deleting a position.
/// The DLC that represents the position will be stored in the database.
/// Errors are handled within the scope of this function.
pub async fn trade(filled: FilledWith) -> Result<()> {
    let order = db::get_order(filled.order_id).context("Could not load order from db")?;

    tracing::debug!(?order, ?filled, "Filling order with id: {}", order.id);

    let trade_params = TradeParams {
        pubkey: ln_dlc::get_node_info()?.pubkey,
        contract_symbol: order.contract_symbol,
        leverage: order.leverage,
        quantity: order.quantity,
        direction: order.direction,
        filled_with: filled,
    };

    let execution_price = trade_params
        .average_execution_price()
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

/// Update the position once an order was submitted
///
/// If the new order submitted is an order that closes the current position, then the position will
/// be updated to `Closing` state.
pub fn update_position_after_order_submitted(submitted_order: Order) -> Result<()> {
    if let Some(position) = db::get_positions()?.first() {
        // closing the position
        if position.direction == submitted_order.direction.opposite()
            && position.quantity == submitted_order.quantity
        {
            db::update_position_state(position.contract_symbol, PositionState::Closing)?;
            event::publish(&EventInternal::PositionUpdateNotification(position.clone()));
        } else {
            bail!("Currently not possible to extend or reduce a position, you can only close the position with a counter-order");
        }
    }

    Ok(())
}

/// Create a position after the creation of a DLC channel.
pub fn update_position_after_dlc_creation(filled_order: Order, collateral: u64) -> Result<()> {
    ensure!(
        db::get_positions()?.is_empty(),
        "Cannot create a position if one is already open"
    );

    tracing::debug!(order = ?filled_order, %collateral, "Creating position after DLC channel creation");

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

    Ok(())
}

/// Delete a position after closing a DLC channel.
pub fn update_position_after_dlc_closure(filled_order: Order) -> Result<()> {
    tracing::debug!(?filled_order, "Removing position after DLC channel closure");

    if db::get_positions()?.is_empty() {
        tracing::warn!("No position to remove");
    }

    db::delete_positions()?;

    event::publish(&EventInternal::PositionCloseNotification(
        ContractSymbol::BtcUsd,
    ));

    Ok(())
}

pub fn price_update(prices: Prices) -> Result<()> {
    event::publish(&EventInternal::PriceUpdateNotification(prices));
    Ok(())
}
