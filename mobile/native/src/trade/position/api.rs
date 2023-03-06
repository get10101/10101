use crate::common::api::Direction;
use crate::trade::position::PositionStateTrade;
use crate::trade::position::PositionTrade;
use flutter_rust_bridge::frb;
use trade::ContractSymbol;

#[frb]
#[derive(Debug, Clone)]
pub enum PositionState {
    /// The position is open
    ///
    /// Open in the sense, that there is an active position that is being rolled-over.
    /// Note that a "closed" position does not exist, but is just removed.
    /// During the process of getting closed (after creating the counter-order that will wipe out
    /// the position), the position is in state "Closing".
    ///
    /// Transitions:
    /// Open->Closing
    Open,
    /// The position is in the process of being closed
    ///
    /// The user has created an order that will wipe out the position.
    /// Once this order has been filled the "closed" the position is not shown in the user
    /// interface, so we don't have a "closed" state because no position data will be provided to
    /// the user interface.
    Closing,
}

#[frb]
#[derive(Debug, Clone)]
pub struct Position {
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub average_entry_price: f64,
    pub liquidation_price: f64,
    /// The unrealized PL can be positive or negative
    pub unrealized_pnl: i64,
    pub position_state: PositionState,
    pub collateral: u64,
}

impl From<PositionStateTrade> for PositionState {
    fn from(value: PositionStateTrade) -> Self {
        match value {
            PositionStateTrade::Open => PositionState::Open,
            PositionStateTrade::Closing => PositionState::Closing,
        }
    }
}

impl From<PositionTrade> for Position {
    fn from(value: PositionTrade) -> Self {
        Position {
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            average_entry_price: value.average_entry_price,
            liquidation_price: value.liquidation_price,
            unrealized_pnl: value.unrealized_pnl,
            position_state: value.position_state.into(),
            collateral: value.collateral,
        }
    }
}
