use crate::db::positions::ContractSymbol;
use crate::orderbook::db::custom_types::Direction;
use crate::schema::trades;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use diesel::prelude::*;
use hex::FromHex;
use lightning::ln::PaymentHash;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = trades)]
struct Trade {
    id: i32,
    position_id: i32,
    contract_symbol: ContractSymbol,
    trader_pubkey: String,
    quantity: f32,
    trader_leverage: f32,
    collateral: i64,
    direction: Direction,
    average_price: f32,
    timestamp: OffsetDateTime,
    fee_payment_hash: String,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = trades)]
struct NewTrade {
    position_id: i32,
    contract_symbol: ContractSymbol,
    trader_pubkey: String,
    quantity: f32,
    trader_leverage: f32,
    collateral: i64,
    direction: Direction,
    average_price: f32,
    pub fee_payment_hash: String,
}

pub fn insert(
    conn: &mut PgConnection,
    trade: crate::trade::models::NewTrade,
) -> Result<crate::trade::models::Trade> {
    let trade: Trade = diesel::insert_into(trades::table)
        .values(NewTrade::from(trade))
        .get_result(conn)?;

    Ok(trade.into())
}

/// Returns the position by trader pub key
pub fn is_payment_hash_registered_as_trade_fee(
    conn: &mut PgConnection,
    payment_hash: PaymentHash,
) -> QueryResult<bool> {
    let payment_hash = payment_hash.0.to_hex();

    let trade = trades::table
        .filter(trades::fee_payment_hash.eq(payment_hash))
        .first::<Trade>(conn)
        .optional()?;

    Ok(trade.is_some())
}

impl From<crate::trade::models::NewTrade> for NewTrade {
    fn from(value: crate::trade::models::NewTrade) -> Self {
        NewTrade {
            position_id: value.position_id,
            contract_symbol: value.contract_symbol.into(),
            trader_pubkey: value.trader_pubkey.to_string(),
            quantity: value.quantity,
            trader_leverage: value.trader_leverage,
            collateral: value.coordinator_margin,
            direction: value.direction.into(),
            average_price: value.average_price,
            fee_payment_hash: value.fee_payment_hash.0.to_hex(),
        }
    }
}

impl From<Trade> for crate::trade::models::Trade {
    fn from(value: Trade) -> Self {
        crate::trade::models::Trade {
            id: value.id,
            position_id: value.position_id,
            contract_symbol: value.contract_symbol.into(),
            trader_pubkey: PublicKey::from_str(value.trader_pubkey.as_str())
                .expect("public key to decode"),
            quantity: value.quantity,
            trader_leverage: value.trader_leverage,
            collateral: value.collateral,
            direction: value.direction.into(),
            average_price: value.average_price,
            timestamp: value.timestamp,
            fee_payment_hash: PaymentHash(
                <[u8; 32]>::from_hex(value.fee_payment_hash).expect("payment hash to decode"),
            ),
        }
    }
}
