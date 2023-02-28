use crate::ln_dlc;
use crate::trade::position::PositionStateTrade;
use crate::trade::position::PositionTrade;
use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use anyhow::Result;
use trade::TradeParams;

/// Sets up a trade with the counterparty
///
/// In a success scenario this results in creating, updating or deleting a position.
/// The DLC that represents the position will be stored in the database.
/// Errors are handled within the scope of this function.
pub async fn trade(trade_params: TradeParams) -> Result<()> {
    ln_dlc::trade(trade_params).await?;

    // TODO: Failure -> Either we send out an event that notifies others (i.e. the order handler)
    //          that this fails, or we just write the failure state to the database here and then
    //          send out an event that the order failed.

    // TODO: Save / update position info in the database (so it is available once the DLC was
    // created and we can construct the position update for the user interface. -> No position
    // yet? Create one from the DLC -> There is an open position? Update it with the new
    // parameters according to the DLC -> There was a position, but the current trade closes it?
    // Delete the position; move relevant position information to a table that keeps the
    // history.

    Ok(())
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
        collateral: 2000,
    };

    Ok(vec![dummy_position])
}
