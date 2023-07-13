use bitcoin::secp256k1::PublicKey;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Debug)]
pub struct NewTrade {
    pub position_id: i32,
    pub contract_symbol: ContractSymbol,
    pub trader_pubkey: PublicKey,
    pub quantity: f32,
    pub leverage: f32,
    pub collateral: i64,
    pub direction: Direction,
    pub average_price: f32,
}

#[derive(Debug)]
pub struct Trade {
    pub id: i32,
    pub position_id: i32,
    pub contract_symbol: ContractSymbol,
    pub trader_pubkey: PublicKey,
    pub quantity: f32,
    pub leverage: f32,
    pub collateral: i64,
    pub direction: Direction,
    pub average_price: f32,
    pub timestamp: OffsetDateTime,
}
