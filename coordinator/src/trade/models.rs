use bitcoin::secp256k1::PublicKey;
use lightning::ln::PaymentHash;
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
    // TODO: Consider removing this since it doesn't make sense with all kinds of trades.
    pub coordinator_margin: i64,
    pub direction: Direction,
    pub average_price: f32,
    pub dlc_expiry_timestamp: Option<OffsetDateTime>,
}

#[derive(Debug)]
pub struct Trade {
    pub id: i32,
    pub position_id: i32,
    pub contract_symbol: ContractSymbol,
    pub trader_pubkey: PublicKey,
    pub quantity: f32,
    pub trader_leverage: f32,
    // TODO: Consider removing this since it doesn't make sense with all kinds of trades.
    pub collateral: i64,
    pub direction: Direction,
    pub average_price: f32,
    // We need this for position resizing so that we can set up the DLC channel using the expiry
    // timestamp specified in the `TradeParams`. It should probably go in a different table since
    // it's not part of the trade model.
    pub dlc_expiry_timestamp: Option<OffsetDateTime>,
    pub timestamp: OffsetDateTime,
    pub fee_payment_hash: PaymentHash,
}
