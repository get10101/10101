use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Debug)]
pub struct NewTrade {
    pub position_id: i32,
    pub contract_symbol: ContractSymbol,
    pub trader_pubkey: PublicKey,
    pub quantity: f32,
    pub trader_leverage: f32,
    pub trader_direction: Direction,
    pub average_price: f32,
    pub order_matching_fee: Amount,
    pub trader_realized_pnl_sat: Option<i64>,
}

#[derive(Debug)]
pub struct Trade {
    pub id: i32,
    pub position_id: i32,
    pub contract_symbol: ContractSymbol,
    pub trader_pubkey: PublicKey,
    pub quantity: f32,
    pub trader_leverage: f32,
    pub direction: Direction,
    pub average_price: f32,
    pub timestamp: OffsetDateTime,
    pub order_matching_fee: Amount,
    pub trader_realized_pnl_sat: Option<i64>,
}
