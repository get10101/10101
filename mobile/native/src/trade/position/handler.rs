use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order;
use crate::trade::order::Order;
use crate::trade::order::OrderState;
use crate::trade::order::OrderType;
use crate::trade::position::compute_relative_contracts;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use coordinator_commons::TradeParams;
use orderbook_commons::FilledWith;
use orderbook_commons::Prices;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
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
        pubkey: ln_dlc::get_node_pubkey(),
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

    // If we have a position _and_ the order is not closing the position (i.e. the contracts between
    // position and order do not match), we must be resizing the position.

    if let Some(Position {
        quantity,
        position_state: PositionState::Open,
        direction,
        contract_symbol,
        ..
    }) = get_positions()?.first()
    {
        let position_contracts_relative = compute_relative_contracts(*quantity, *direction);
        let trade_contracts_relative =
            compute_relative_contracts(trade_params.quantity, trade_params.direction);

        if *contract_symbol == trade_params.contract_symbol
            && position_contracts_relative + trade_contracts_relative != Decimal::ZERO
        {
            set_position_state(PositionState::Resizing)?;
        }
    }

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
        stable: order.stable,
    };

    db::insert_order(order)?;

    event::publish(&EventInternal::OrderUpdateNotification(order));

    let trade_params = TradeParams {
        pubkey: ln_dlc::get_node_pubkey(),
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
pub async fn rollover(contract_id: Option<String>) -> Result<()> {
    ln_dlc::rollover(contract_id).await
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

/// If the submitted order would close the current [`Position`], return the [`position`].
pub fn get_position_matching_order(order: &Order) -> Result<Option<Position>> {
    match db::get_positions()?.first() {
        Some(position)
            if position.direction != order.direction && position.quantity == order.quantity =>
        {
            Ok(Some(position.clone()))
        }
        _ => Ok(None),
    }
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
    let (position, trades) = match db::get_positions()?.first() {
        None => {
            tracing::debug!(
                order = ?filled_order,
                %collateral,
                "Creating position after DLC channel creation"
            );

            let (position, trade) = Position::new_open(filled_order, collateral, expiry);

            tracing::info!(?trade, ?position, "Position created");

            db::insert_position(position.clone())?;

            (position, vec![trade])
        }
        Some(
            position @ Position {
                position_state: PositionState::Resizing,
                ..
            },
        ) => {
            tracing::info!("Calculating new position after DLC channel has been resized");

            let (position, trades) =
                position
                    .clone()
                    .apply_order(filled_order, expiry, collateral)?;

            let position = position.context("Resized position has vanished")?;

            db::update_position(position.clone())?;

            (position, trades)
        }
        Some(position) => {
            bail!(
                "Cannot resize position in state {:?}",
                position.position_state
            );
        }
    };

    for trade in trades {
        db::insert_trade(trade)?;
    }

    event::publish(&EventInternal::PositionUpdateNotification(position));

    Ok(())
}

/// Delete a position after closing a DLC channel.
pub fn update_position_after_dlc_closure(filled_order: Option<Order>) -> Result<()> {
    tracing::debug!(?filled_order, "Removing position after DLC channel closure");

    let position = match db::get_positions()?.as_slice() {
        [position] => position.clone(),
        [position, ..] => {
            tracing::warn!("Found more than one position. Taking the first one");
            position.clone()
        }
        [] => {
            tracing::warn!("No position to remove");
            return Ok(());
        }
    };

    if let Some(filled_order) = filled_order {
        tracing::debug!(
            ?position,
            ?filled_order,
            "Calculating closing trades for position"
        );

        // After closing the DLC channel we do not need to update the position's expiry anymore.
        let expiry = position.expiry;
        // The collateral is 0 since the DLC channel has been closed.
        let actual_collateral = 0;
        let (new_position, trades) =
            position.apply_order(filled_order, expiry, actual_collateral)?;

        tracing::debug!(?trades, "Calculated closing trades");

        if let Some(new_position) = new_position {
            tracing::warn!(
                ?new_position,
                "Expected computed position to vanish after applying closing order"
            );
        }

        for trade in trades {
            db::insert_trade(trade)?;
        }
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
