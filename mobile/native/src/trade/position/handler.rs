use crate::calculations::calculate_liquidation_price;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order;
use crate::trade::order::Order;
use crate::trade::order::OrderState;
use crate::trade::order::OrderType;
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
use time::OffsetDateTime;
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
        .to_f32()
        .expect("to fit into f32");
    order::handler::order_filling(order.id, execution_price)
        .context("Could not update order to filling")?;

    if let Err((reason, e)) = ln_dlc::trade(trade_params).await {
        order::handler::order_failed(Some(order.id), reason, e)
            .context("Could not set order to failed")?;
    }

    Ok(())
}

/// Executes an async trade from the orderbook / coordinator. e.g. this will happen if the position
/// expires.
pub async fn async_trade(order: orderbook_commons::Order, filled_with: FilledWith) -> Result<()> {
    let order_type = match order.order_type {
        orderbook_commons::OrderType::Market => OrderType::Market,
        orderbook_commons::OrderType::Limit => OrderType::Limit {
            price: order.price.to_f32().expect("to fit into f32"),
        },
    };

    let execution_price = filled_with
        .average_execution_price()
        .to_f32()
        .expect("to fit into f32");
    let order = Order {
        id: order.id,
        leverage: order.leverage,
        quantity: order.quantity.to_f32().expect("to fit into f32"),
        contract_symbol: order.contract_symbol,
        direction: order.direction,
        order_type,
        state: OrderState::Filling { execution_price },
        creation_timestamp: order.timestamp,
        order_expiry_timestamp: order.expiry,
        reason: order.order_reason.into(),
    };

    db::insert_order(order)?;

    event::publish(&EventInternal::OrderUpdateNotification(order));

    let trade_params = TradeParams {
        pubkey: ln_dlc::get_node_info()?.pubkey,
        contract_symbol: order.contract_symbol,
        leverage: order.leverage,
        quantity: order.quantity,
        direction: order.direction,
        filled_with,
    };

    if let Err((reason, e)) = ln_dlc::trade(trade_params).await {
        order::handler::order_failed(Some(order.id), reason, e)
            .context("Could not set order to failed")?;
    }

    Ok(())
}

/// Rollover dlc to new expiry timestamp
pub async fn rollover() -> Result<()> {
    ln_dlc::rollover().await
}

/// Fetch the positions from the database
pub fn get_positions() -> Result<Vec<Position>> {
    db::get_positions()
}

/// Update the position once an order was submitted
///
/// If the new order submitted is an order that closes the current position, then the position will
/// be updated to `Closing` state.
pub fn update_position_after_order_submitted(submitted_order: &Order) -> Result<()> {
    if let Some(position) = get_position_matching_order(submitted_order)? {
        db::update_position_state(position.contract_symbol, PositionState::Closing)?;
        let mut position = position;
        position.position_state = PositionState::Closing;
        event::publish(&EventInternal::PositionUpdateNotification(position));
    }
    Ok(())
}

/// Returns the position that would be closed by submitted, if there is any
pub fn get_position_matching_order(order: &Order) -> Result<Option<Position>> {
    Ok(if let Some(position) = db::get_positions()?.first() {
        // closing the position
        ensure!(position.direction == order.direction.opposite()
                && position.quantity == order.quantity, "Currently not possible to extend or reduce a position, you can only close the position with a counter-order");
        Some(position.clone())
    } else {
        None
    })
}

/// Sets the position to the given state
pub fn set_position_state(state: PositionState) -> Result<()> {
    if let Some(position) = db::get_positions()?.first() {
        db::update_position_state(position.contract_symbol, state)?;
        let mut position = position.clone();
        position.position_state = state;
        event::publish(&EventInternal::PositionUpdateNotification(position));
    }

    Ok(())
}

pub fn rollover_position(expiry_timestamp: OffsetDateTime) -> Result<()> {
    if let Some(position) = db::get_positions()?.first() {
        tracing::debug!("Setting position to rollover");
        db::rollover_position(position.contract_symbol, expiry_timestamp)?;
        let mut position = position.clone();
        position.position_state = PositionState::Rollover;
        position.expiry = expiry_timestamp;
        event::publish(&EventInternal::PositionUpdateNotification(position));
    } else {
        bail!("Cannot rollover non-existing position");
    }

    Ok(())
}

/// Create a position after the creation of a DLC channel.
pub fn update_position_after_dlc_creation(
    filled_order: Order,
    collateral: u64,
    expiry: OffsetDateTime,
) -> Result<()> {
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
        expiry,
        updated: OffsetDateTime::now_utc(),
        created: OffsetDateTime::now_utc(),
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
    tracing::debug!(?prices, "Updating prices");
    event::publish(&EventInternal::PriceUpdateNotification(prices));
    Ok(())
}
