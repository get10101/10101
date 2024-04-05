use crate::db::positions::ContractSymbol;
use crate::orderbook::db::custom_types::Direction;
use crate::schema::trades;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
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
    trader_leverage: f32,
    collateral: i64,
    direction: Direction,
    average_price: f32,
    timestamp: OffsetDateTime,
    order_matching_fee_sat: i64,
    trader_realized_pnl_sat: Option<i64>,
    is_complete: bool,
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
    order_matching_fee_sat: i64,
    trader_realized_pnl_sat: Option<i64>,
    is_complete: bool,
}

pub fn insert(
    conn: &mut PgConnection,
    trade: crate::trade::models::NewTrade,
) -> QueryResult<crate::trade::models::Trade> {
    let trade: Trade = diesel::insert_into(trades::table)
        .values(NewTrade::from(trade))
        .get_result(conn)?;

    Ok(trade.into())
}

pub fn mark_as_completed(conn: &mut PgConnection, position_id: i32) -> QueryResult<()> {
    let trade = trades::table
        .filter(trades::position_id.eq(position_id))
        .order_by(trades::id.desc())
        .first::<Trade>(conn)
        .optional()?
        .ok_or(diesel::result::Error::NotFound)?;

    let affected_rows = diesel::update(trades::table)
        .filter(trades::id.eq(trade.id))
        .set(trades::is_complete.eq(true))
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}

pub fn get_latest_for_position(
    conn: &mut PgConnection,
    position_id: i32,
) -> Result<Option<crate::trade::models::Trade>> {
    let trade = trades::table
        .filter(trades::position_id.eq(position_id))
        .order_by(trades::id.desc())
        .first::<Trade>(conn)
        .optional()?;

    Ok(trade.map(crate::trade::models::Trade::from))
}

pub fn get_trades(
    connection: &mut PgConnection,
    trader_pubkey: PublicKey,
) -> Result<Vec<crate::trade::models::Trade>> {
    let trades: Vec<Trade> = trades::table
        .filter(trades::trader_pubkey.eq(trader_pubkey.to_string()))
        .load::<Trade>(connection)?;

    let trades = trades
        .into_iter()
        .map(crate::trade::models::Trade::from)
        .collect();

    Ok(trades)
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
            direction: value.trader_direction.into(),
            average_price: value.average_price,
            order_matching_fee_sat: value.order_matching_fee.to_sat() as i64,
            trader_realized_pnl_sat: value.trader_realized_pnl_sat,
            is_complete: value.is_complete,
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
            order_matching_fee: Amount::from_sat(value.order_matching_fee_sat as u64),
            trader_realized_pnl_sat: value.trader_realized_pnl_sat,
            is_complete: value.is_complete,
        }
    }
}
