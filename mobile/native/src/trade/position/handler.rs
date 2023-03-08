use crate::calculations::calculate_liquidation_price;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order::Order;
use crate::trade::order::OrderState;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use anyhow::Result;
use trade::ContractSymbol;
use trade::Direction;
use trade::TradeParams;

/// Sets up a trade with the counterparty
///
/// In a success scenario this results in creating, updating or deleting a position.
/// The DLC that represents the position will be stored in the database.
/// Errors are handled within the scope of this function.
pub async fn trade(trade_params: TradeParams) -> Result<()> {
    db::update_order_state(
        trade_params.order_id,
        OrderState::Filling {
            execution_price: trade_params.execution_price,
        },
    )?;

    ln_dlc::trade(trade_params).await?;

    // TODO: Failure -> Either we send out an event that notifies others (i.e. the order handler)
    //          that this fails, or we just write the failure state to the database here and then
    //          send out an event that the order failed.

    Ok(())
}

/// Fetch the positions from the database
pub async fn get_positions() -> Result<Vec<Position>> {
    // TODO: Fetch position from database

    let dummy_position = Position {
        leverage: 2.0,
        quantity: 10000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        average_entry_price: 20000.0,
        liquidation_price: 14000.0,
        unrealized_pnl: -400,
        position_state: PositionState::Open,
        collateral: 2000,
    };

    Ok(vec![dummy_position])
}

pub fn order_filled(filled_order: Order, collateral: u64) -> Result<()> {
    // TODO: Persist the position
    // TODO: Decide if we have to create or update the position:
    //  Probably best to have a "insert or update" function for the db that returns the position

    // TODO: Edge case: Closing a position: we will have to decide if we remove from the `positions`
    // table or just update the state of the position by introducing a `Closed` state.

    // TODO: Maybe we should change the model to *ensure* that the execution price is present past a
    // certain point; i.e. a `FilledOrder` struct?
    let average_entry_price = filled_order.execution_price().unwrap_or(0.0);

    event::publish(&EventInternal::PositionUpdateNotification(Position {
        leverage: filled_order.leverage,
        quantity: filled_order.quantity,
        contract_symbol: filled_order.contract_symbol,
        direction: filled_order.direction,
        average_entry_price,
        // TODO: Is it correct to use the average entry price to calculate the liquidation price? ->
        // What would that mean in the UI if we already have a position and trade?
        liquidation_price: calculate_liquidation_price(
            average_entry_price,
            filled_order.leverage,
            filled_order.direction,
        ),
        // TODO: PnL calc
        unrealized_pnl: 0,
        position_state: PositionState::Open,
        collateral,
    }));

    Ok(())
}

pub fn is_position_up_to_date(_dlc_collateral: &u64) -> bool {
    // TODO load the position and compare the collateral.
    // Only if there is a position and said position's collateral is the same as the same as the DLC
    // collateral we don't need an update.

    // dummy: at the moment we always update
    false
}
