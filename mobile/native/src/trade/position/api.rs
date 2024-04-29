use crate::trade::position;
use flutter_rust_bridge::frb;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::Direction;

#[frb]
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum PositionState {
    /// The position is open
    ///
    /// Open in the sense, that there is an active position that is being rolled-over.
    /// Note that a "closed" position does not exist, but is just removed.
    /// During the process of getting closed (after creating the counter-order that will wipe out
    /// the position), the position is in state "Closing".
    ///
    /// Transitions:
    /// ->Open
    /// Rollover->Open
    Open,
    /// The position is in the process of being closed
    ///
    /// The user has created an order that will wipe out the position.
    /// Once this order has been filled the "closed" the position is not shown in the user
    /// interface, so we don't have a "closed" state because no position data will be provided to
    /// the user interface.
    /// Transitions:
    /// Open->Closing
    Closing,
    /// The position is in rollover
    ///
    /// This is a technical intermediate state indicating that a rollover is currently in progress.
    ///
    /// Transitions:
    /// Open->Rollover
    Rollover,
    /// The position is being resized.
    ///
    /// Transitions:
    /// Open->Resizing.
    Resizing,
}

#[frb]
#[derive(Debug, Clone)]
pub struct Position {
    pub leverage: f32,
    pub quantity: f32,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub collateral: u64,
    pub expiry: i64,
    pub stable: bool,
}

impl From<position::PositionState> for PositionState {
    fn from(value: position::PositionState) -> Self {
        match value {
            position::PositionState::Open => PositionState::Open,
            position::PositionState::Closing => PositionState::Closing,
            position::PositionState::Rollover => PositionState::Rollover,
            position::PositionState::Resizing => PositionState::Resizing,
        }
    }
}

impl From<position::Position> for Position {
    fn from(value: position::Position) -> Self {
        Position {
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            average_entry_price: value.average_entry_price,
            liquidation_price: value.liquidation_price,
            position_state: value.position_state.into(),
            collateral: value.collateral,
            expiry: value.expiry.unix_timestamp(),
            stable: value.stable,
        }
    }
}
