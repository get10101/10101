use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;

pub mod api;
pub mod handler;
pub mod subscriber;

#[derive(Debug, Clone, PartialEq)]
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
    ///
    /// A position that failed to close should be brought back to the "Open" state.
    ///
    /// Transitions:
    /// Closing->Open
    Closing,
}

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
    pub expiry: OffsetDateTime,
    pub updated: OffsetDateTime,
    pub created: OffsetDateTime,
}
