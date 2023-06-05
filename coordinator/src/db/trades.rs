use crate::db::positions::ContractSymbol;
use crate::orderbook::db::custom_types::Direction;
use crate::schema::trades;
use anyhow::Result;
use autometrics::autometrics;
use diesel::prelude::*;

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = trades)]
pub struct Trade {
    pub position_id: i32,
    pub contract_symbol: ContractSymbol,
    pub trader_pubkey: String,
    pub quantity: f32,
    pub leverage: f32,
    pub collateral: i64,
    pub direction: Direction,
    pub average_price: f32,
}

#[autometrics]
pub fn insert(conn: &mut PgConnection, trade: Trade) -> Result<()> {
    diesel::insert_into(trades::table)
        .values(trade)
        .execute(conn)?;

    Ok(())
}
