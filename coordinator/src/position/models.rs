use bitcoin::secp256k1::PublicKey;
use dlc_manager::ContractId;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Debug, Clone)]
pub struct NewPosition {
    pub contract_symbol: ContractSymbol,
    pub leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub trader: PublicKey,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub collateral: i64,
    pub expiry_timestamp: OffsetDateTime,
    pub temporary_contract_id: ContractId,
}

#[derive(PartialEq, Debug)]
pub enum PositionState {
    Open,
    Closing,
}

#[derive(Debug)]
pub struct Position {
    pub id: i32,
    pub contract_symbol: ContractSymbol,
    pub leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub collateral: i64,
    pub creation_timestamp: OffsetDateTime,
    pub expiry_timestamp: OffsetDateTime,
    pub update_timestamp: OffsetDateTime,
    pub trader: PublicKey,

    /// This is the temporary contract id that is created when the contract is being offered
    ///
    /// We use the temporary contract id because the actual contract id might not be known at that
    /// point. The temporary contract id is propagated to all states until the contract is
    /// closed.
    pub temporary_contract_id: ContractId,
}
