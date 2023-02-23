use crate::event;
use crate::event::EventInternal;
use crate::trade::position::PositionStateTrade;
use crate::trade::position::PositionTrade;
use crate::trade::position::TradeParams;
use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use anyhow::Result;

/// Sets up a trade with the counterparty
///
/// In a success scenario this results in creating, updating or deleting a position.
/// The DLC that represents the position will be stored in the database.
/// Errors are handled within the scope of this function.
pub async fn trade(_trade_params: TradeParams) {
    // TODO: Send trade parameters to coordinator

    // TODO: Success: We have a DLC!
    // TODO: Failure -> Either we send out an event that notifies others (i.e. the order handler)
    //          that this fails, or we just write the failure state to the database here and then
    //          send out an event that the order failed.

    // TODO: Update the position in the database
    // -> No position yet? Create one from the DLC
    // -> There is an open position? Update it with the new parameters according to the DLC
    // -> There was a position, but the current trade closes it? Delete the position; move relevant
    // position information to a table that keeps the history.

    // TODO: Send out position update

    event::publish(&EventInternal::PositionUpdateNotification(PositionTrade {
        leverage: 1.50,
        quantity: 15000.0,
        contract_symbol: ContractSymbolTrade::BtcUsd,
        direction: DirectionTrade::Long,
        average_entry_price: 23400.0,
        liquidation_price: 14500.0,
        unrealized_pnl: 600,
        position_state: PositionStateTrade::Open,
    }));
}

/// Fetch the positions from the database
pub async fn get_positions() -> Result<Vec<PositionTrade>> {
    // TODO: Fetch from database

    let dummy_position = PositionTrade {
        leverage: 2.0,
        quantity: 10000.0,
        contract_symbol: ContractSymbolTrade::BtcUsd,
        direction: DirectionTrade::Long,
        average_entry_price: 20000.0,
        liquidation_price: 14000.0,
        unrealized_pnl: -400,
        position_state: PositionStateTrade::Open,
    };

    Ok(vec![dummy_position])
}
