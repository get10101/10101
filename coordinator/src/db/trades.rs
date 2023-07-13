use crate::db::positions::ContractSymbol;
use crate::orderbook::db::custom_types::Direction;
use crate::schema::trades;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use diesel::prelude::*;
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
    leverage: f32,
    collateral: i64,
    direction: Direction,
    average_price: f32,
    timestamp: OffsetDateTime,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = trades)]
struct NewTrade {
    position_id: i32,
    contract_symbol: ContractSymbol,
    trader_pubkey: String,
    quantity: f32,
    leverage: f32,
    collateral: i64,
    direction: Direction,
    average_price: f32,
}

#[autometrics]
pub fn insert(
    conn: &mut PgConnection,
    trade: crate::trade::models::NewTrade,
) -> Result<crate::trade::models::Trade> {
    let trade: Trade = diesel::insert_into(trades::table)
        .values(NewTrade::from(trade))
        .get_result(conn)?;

    Ok(trade.into())
}

impl From<crate::trade::models::NewTrade> for NewTrade {
    fn from(value: crate::trade::models::NewTrade) -> Self {
        NewTrade {
            position_id: value.position_id,
            contract_symbol: value.contract_symbol.into(),
            trader_pubkey: value.trader_pubkey.to_string(),
            quantity: value.quantity,
            leverage: value.leverage,
            collateral: value.collateral,
            direction: value.direction.into(),
            average_price: value.average_price,
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
            leverage: value.leverage,
            collateral: value.collateral,
            direction: value.direction.into(),
            average_price: value.average_price,
            timestamp: value.timestamp,
        }
    }
}
