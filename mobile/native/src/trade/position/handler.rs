use crate::calculations::calculate_liquidation_price;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order;
use crate::trade::order::Order;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
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
    let order = db::get_order(filled.order_id)?;

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
    order::handler::order_filling(order.id, execution_price)?;

    if let Err((reason, e)) = ln_dlc::trade(trade_params).await {
        order::handler::order_failed(Some(order.id), reason, e)?;
    }

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
