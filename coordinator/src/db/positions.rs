use crate::orderbook::db::custom_types::Direction;
use crate::schema::positions;
use crate::schema::sql_types::ContractSymbolType;
use crate::schema::sql_types::PositionStateType;
use anyhow::bail;
use anyhow::Result;
use diesel::prelude::*;
use diesel::query_builder::QueryId;
use diesel::result::QueryResult;
use diesel::AsExpression;
use diesel::FromSqlRow;
use std::any::TypeId;
use time::OffsetDateTime;

#[derive(Queryable, Debug, Clone)]
pub(crate) struct Position {
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
    pub trader_pubkey: String,
}

impl Position {
    /// Returns the position by id
    pub fn get_open_position_by_trader(
        conn: &mut PgConnection,
        trader: String,
    ) -> QueryResult<Option<crate::position::models::Position>> {
        let x = positions::table
            .filter(positions::trader_pubkey.eq(trader))
            .filter(positions::position_state.eq(PositionState::Open))
            .first::<Position>(conn)
            .optional()?;

        Ok(x.map(crate::position::models::Position::from))
    }

    /// sets the status of all open position to closing (note, we expect that number to be always
    /// exactly 1)
    pub fn set_open_position_to_closing(
        conn: &mut PgConnection,
        trader_pubkey: String,
    ) -> Result<()> {
        let effected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(positions::position_state.eq(PositionState::Open))
            .set(positions::position_state.eq(PositionState::Closing))
            .execute(conn)?;

        if effected_rows == 0 {
            bail!("Could not update position to Closing for {trader_pubkey}")
        }

        Ok(())
    }

    /// inserts the given position into the db. Returns the position if successful
    pub fn insert(
        conn: &mut PgConnection,
        new_position: crate::position::models::NewPosition,
    ) -> Result<crate::position::models::Position> {
        let position: Position = diesel::insert_into(positions::table)
            .values(NewPosition::from(new_position))
            .get_result(conn)?;

        Ok(position.into())
    }
}

impl From<Position> for crate::position::models::Position {
    fn from(value: Position) -> Self {
        crate::position::models::Position {
            id: value.id,
            contract_symbol: trade::ContractSymbol::from(value.contract_symbol),
            leverage: value.leverage,
            quantity: value.quantity,
            direction: trade::Direction::from(value.direction),
            average_entry_price: value.average_entry_price,
            liquidation_price: value.liquidation_price,
            position_state: crate::position::models::PositionState::from(value.position_state),
            collateral: value.collateral,
            creation_timestamp: value.creation_timestamp,
            expiry_timestamp: value.expiry_timestamp,
            update_timestamp: value.update_timestamp,
            trader: value.trader_pubkey.parse().expect("to be valid public key"),
        }
    }
}

#[derive(Insertable, Debug, PartialEq)]
#[diesel(table_name = positions)]
struct NewPosition {
    pub contract_symbol: ContractSymbol,
    pub leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub collateral: i64,
    pub expiry_timestamp: OffsetDateTime,
    pub trader_pubkey: String,
}

impl From<crate::position::models::NewPosition> for NewPosition {
    fn from(value: crate::position::models::NewPosition) -> Self {
        NewPosition {
            contract_symbol: ContractSymbol::from(value.contract_symbol),
            leverage: value.leverage,
            quantity: value.quantity,
            direction: Direction::from(value.direction),
            average_entry_price: value.average_entry_price,
            liquidation_price: value.liquidation_price,
            position_state: PositionState::Open,
            collateral: value.collateral,
            expiry_timestamp: value.expiry_timestamp,
            trader_pubkey: value.trader.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = PositionStateType)]
pub enum PositionState {
    Open,
    Closing,
}

impl QueryId for PositionStateType {
    type QueryId = PositionStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl From<PositionState> for crate::position::models::PositionState {
    fn from(value: PositionState) -> Self {
        match value {
            PositionState::Open => crate::position::models::PositionState::Open,
            PositionState::Closing => crate::position::models::PositionState::Closing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = ContractSymbolType)]
pub enum ContractSymbol {
    BtcUsd,
}

impl QueryId for ContractSymbolType {
    type QueryId = ContractSymbolType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl From<ContractSymbol> for trade::ContractSymbol {
    fn from(value: ContractSymbol) -> Self {
        match value {
            ContractSymbol::BtcUsd => trade::ContractSymbol::BtcUsd,
        }
    }
}

impl From<trade::ContractSymbol> for ContractSymbol {
    fn from(value: trade::ContractSymbol) -> Self {
        match value {
            trade::ContractSymbol::BtcUsd => ContractSymbol::BtcUsd,
        }
    }
}
