use crate::calculations::calculate_margin;
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
use commons::FilledWith;
use commons::Prices;
use commons::TradeAndChannelParams;
use commons::TradeParams;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::ContractSymbol;

/// Sets up a trade with the counterparty.
///
/// In a success scenario, this results in creating, updating or deleting a position.
pub async fn trade(filled: FilledWith) -> Result<()> {
    let order = db::get_order(filled.order_id).context("Could not load order from db")?;

    tracing::debug!(?order, ?filled, "Filling order");

    let (coordinator_reserve, trader_reserve) =
        match db::get_channel_opening_params_by_order_id(filled.order_id)
            .context("Could not load channel-opening params from db")?
        {
            Some(channel_opening_params) => {
                tracing::debug!(?channel_opening_params, "Found channel-opening parameters");

                (
                    Some(channel_opening_params.coordinator_reserve),
                    Some(channel_opening_params.trader_reserve),
                )
            }
            None => (None, None),
        };

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

    let params = TradeAndChannelParams {
        trade_params,
        coordinator_reserve,
        trader_reserve,
    };

    if let Err((reason, e)) = ln_dlc::trade(params).await {
        order::handler::order_failed(Some(order.id), reason, e)
            .context("Could not set order to failed")?;
    }

    Ok(())
}

/// Executes an async trade from the orderbook / coordinator. e.g. this will happen if the position
/// expires.
pub async fn async_trade(order: commons::Order, filled_with: FilledWith) -> Result<()> {
    let order_type = match order.order_type {
        commons::OrderType::Market => OrderType::Market,
        commons::OrderType::Limit => OrderType::Limit {
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
        failure_reason: None,
    };

    db::insert_order(order.clone())?;

    event::publish(&EventInternal::OrderUpdateNotification(order.clone()));

    let trade_params = TradeParams {
        pubkey: ln_dlc::get_node_pubkey(),
        contract_symbol: order.contract_symbol,
        leverage: order.leverage,
        quantity: order.quantity,
        direction: order.direction,
        filled_with,
    };

    let params = TradeAndChannelParams {
        trade_params,
        // An "async" trade is only triggered to close and expired position. We don't need
        // channel-opening parameters to close a position.
        coordinator_reserve: None,
        trader_reserve: None,
    };

    if let Err((reason, e)) = ln_dlc::trade(params).await {
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

/// A channel renewal could be triggered for:
///
/// - Rolling over (no offer associated).
/// - Opening a new position.
/// - Resizing a new position.
pub fn handle_channel_renewal_offer(expiry_timestamp: OffsetDateTime) -> Result<()> {
    // FIXME: This won't always be true once we reintroduce position resizing.
    if let Some(position) = db::get_positions()?.first() {
        tracing::debug!("Setting position to rollover");
        db::rollover_position(position.contract_symbol, expiry_timestamp)?;
        let mut position = position.clone();
        position.position_state = PositionState::Rollover;
        position.expiry = expiry_timestamp;
        event::publish(&EventInternal::PositionUpdateNotification(position));
    } else {
        // If we have no position we must be opening a new one.
        tracing::info!("Received channel renewal proposal to open new position");
    }

    Ok(())
}

/// Create a position after creating or updating a DLC channel.
pub fn update_position_after_dlc_channel_creation_or_update(
    filled_order: Order,
    expiry: OffsetDateTime,
) -> Result<()> {
    let execution_price = filled_order
        .execution_price()
        .expect("filled order to have a price");

    // This used to be determined by the DLC collateral, but we now allow part of
    // the DLC collateral to be excluded from the trade (the collateral reserve).
    let margin = calculate_margin(
        execution_price,
        filled_order.quantity,
        filled_order.leverage,
    );

    let (position, trades) = match db::get_positions()?.first() {
        None => {
            tracing::debug!(
                order = ?filled_order,
                %margin,
                "Creating position after DLC channel creation"
            );

            let (position, trade) = Position::new_open(filled_order, margin, expiry);

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

            let (position, trades) = position.clone().apply_order(filled_order, expiry, margin)?;

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
