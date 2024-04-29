use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::trade::order::Order;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use crate::trade::trades::handler::new_trade;
use crate::trade::FundingFeeEvent;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::Direction;

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
        let position = db::update_position_state(position.contract_symbol, PositionState::Closing)?;
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
            Ok(Some(*position))
        }
        _ => Ok(None),
    }
}

/// Set the position to the given [`PositionState`].
pub fn set_position_state(state: PositionState) -> Result<()> {
    if let Some(position) = db::get_positions()?.first() {
        let position = db::update_position_state(position.contract_symbol, state)?;
        event::publish(&EventInternal::PositionUpdateNotification(position));
    }

    Ok(())
}

pub fn handle_renew_offer() -> Result<()> {
    if let Some(position) = db::get_positions()?.first() {
        tracing::debug!("Received renew offer to resize position");

        let position =
            db::update_position_state(position.contract_symbol, PositionState::Resizing)?;

        event::publish(&EventInternal::PositionUpdateNotification(position));
    } else {
        // If we have no position, we must be opening a new one.
        tracing::info!("Received renew offer to open new position");
    }

    Ok(())
}

pub fn handle_rollover_offer(
    expiry_timestamp: OffsetDateTime,
    funding_fee_events: &[FundingFeeEvent],
) -> Result<()> {
    tracing::debug!("Setting position state to rollover");

    let positions = &db::get_positions()?;
    let position = positions.first().context("No position to roll over")?;

    // TODO: Update the `expiry_timestamp` only after the rollover protocol is finished. We only do
    // it so that we don't have to store the `expiry_timestamp` in the database.
    let position = position
        .start_rollover(expiry_timestamp)
        .apply_funding_fee_events(funding_fee_events)?;

    db::start_position_rollover(position)?;

    event::publish(&EventInternal::PositionUpdateNotification(position));

    Ok(())
}

/// Update position after completing rollover protocol.
pub fn update_position_after_rollover() -> Result<Position> {
    tracing::debug!("Setting position state from rollover back to open");

    let positions = &db::get_positions()?;
    let position = positions
        .first()
        .context("No position to finish rollover")?;

    let position = position.finish_rollover();

    db::finish_position_rollover(position)?;

    event::publish(&EventInternal::PositionUpdateNotification(position));

    Ok(position)
}

/// The app will sometimes receive [`FundingFeeEvent`]s from the coordinator which are not directly
/// linked to a channel update. These need to be applied to the [`Position`] to keep it in sync with
/// the coordinator.
pub fn handle_funding_fee_events(funding_fee_events: &[FundingFeeEvent]) -> Result<()> {
    if funding_fee_events.is_empty() {
        return Ok(());
    }

    tracing::debug!(
        ?funding_fee_events,
        "Applying funding fee events to position"
    );

    let positions = &db::get_positions()?;
    let position = positions
        .first()
        .context("No position to apply funding fee events")?;

    let position = position.apply_funding_fee_events(funding_fee_events)?;

    db::update_position(position)?;

    event::publish(&EventInternal::PositionUpdateNotification(position));

    Ok(())
}

/// Create or insert a position after filling an order.
pub fn update_position_after_dlc_channel_creation_or_update(
    filled_order: Order,
    expiry: OffsetDateTime,
) -> Result<()> {
    let (position, trades) = match db::get_positions()?.first() {
        None => {
            // TODO: This log message seems to assume that we can only reach this branch if the
            // channel was just created. Is that true?
            tracing::debug!(
                order = ?filled_order,
                "Creating position after DLC channel creation"
            );

            let (position, trade) = Position::new_open(filled_order, expiry);

            tracing::info!(?trade, ?position, "Position created");

            db::insert_position(position)?;

            (position, vec![trade])
        }
        Some(
            position @ Position {
                position_state: PositionState::Resizing,
                ..
            },
        ) => {
            tracing::info!("Calculating new position after DLC channel has been resized");

            let (position, trades) = position.apply_order(filled_order, expiry)?;

            let position = position.context("Resized position has vanished")?;

            db::update_position(position)?;

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
        new_trade(trade)?;
    }

    event::publish(&EventInternal::PositionUpdateNotification(position));

    Ok(())
}

/// Delete a position after closing a DLC channel.
pub fn update_position_after_dlc_closure(filled_order: Order) -> Result<()> {
    tracing::debug!(?filled_order, "Removing position after DLC channel closure");

    let positions = &db::get_positions()?;
    let position = match positions.as_slice() {
        [position] => position,
        [position, ..] => {
            tracing::warn!("Found more than one position. Taking the first one");
            position
        }
        [] => {
            tracing::warn!("No position to remove");
            return Ok(());
        }
    };

    tracing::debug!(
        ?position,
        ?filled_order,
        "Calculating closing trades for position"
    );

    // After closing the DLC channel we do not need to update the position's expiry anymore.
    let expiry = position.expiry;
    let (new_position, trades) = position.apply_order(filled_order, expiry)?;

    tracing::debug!(?trades, "Calculated closing trades");

    if let Some(new_position) = new_position {
        tracing::warn!(
            ?new_position,
            "Expected computed position to vanish after applying closing order"
        );
    }

    for trade in trades {
        new_trade(trade)?;
    }

    db::delete_positions()?;

    event::publish(&EventInternal::PositionCloseNotification(
        ContractSymbol::BtcUsd,
    ));

    Ok(())
}

pub fn price_update(price: Decimal, direction: Direction) {
    match direction {
        Direction::Long => {
            tracing::debug!(?price, "Updating long price");
        }
        Direction::Short => {
            tracing::debug!(?price, "Updating short price");
            event::publish(&EventInternal::AskPriceUpdateNotification(price));
        }
    }
}
