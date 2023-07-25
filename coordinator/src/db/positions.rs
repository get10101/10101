use crate::orderbook::db::custom_types::Direction;
use crate::schema::positions;
use crate::schema::sql_types::ContractSymbolType;
use crate::schema::sql_types::PositionStateType;
use anyhow::bail;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::hashes::hex::ToHex;
use diesel::prelude::*;
use diesel::query_builder::QueryId;
use diesel::result::QueryResult;
use diesel::AsExpression;
use diesel::FromSqlRow;
use dlc_manager::ContractId;
use hex::FromHex;
use std::any::TypeId;
use time::OffsetDateTime;

#[derive(Queryable, Debug, Clone)]
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
    pub trader_pubkey: String,
    pub temporary_contract_id: Option<String>,
    pub realized_pnl_sat: Option<i64>,
    pub unrealized_pnl_sat: Option<i64>,
    pub closing_price: Option<f32>,
}

impl Position {
    /// Returns the position by trader pub key
    #[autometrics]
    pub fn get_open_position_by_trader(
        conn: &mut PgConnection,
        trader_pubkey: String,
    ) -> QueryResult<Option<crate::position::models::Position>> {
        let x = positions::table
            .filter(positions::trader_pubkey.eq(trader_pubkey))
            .filter(positions::position_state.eq(PositionState::Open))
            .first::<Position>(conn)
            .optional()?;

        Ok(x.map(crate::position::models::Position::from))
    }

    #[autometrics]
    pub fn get_all_open_positions(
        conn: &mut PgConnection,
    ) -> QueryResult<Vec<crate::position::models::Position>> {
        let positions = positions::table
            .filter(positions::position_state.eq(PositionState::Open))
            .load::<Position>(conn)?;

        let positions = positions
            .into_iter()
            .map(crate::position::models::Position::from)
            .collect();

        Ok(positions)
    }

    pub fn get_all_open_or_closing_positions(
        conn: &mut PgConnection,
    ) -> QueryResult<Vec<crate::position::models::Position>> {
        let positions = positions::table
            .filter(
                positions::position_state
                    .eq(PositionState::Open)
                    .or(positions::position_state.eq(PositionState::Closing)),
            )
            .load::<Position>(conn)?;

        let positions = positions
            .into_iter()
            .map(crate::position::models::Position::from)
            .collect();

        Ok(positions)
    }

    /// sets the status of all open position to closing (note, we expect that number to be always
    /// exactly 1)
    pub fn set_open_position_to_closing(
        conn: &mut PgConnection,
        trader_pubkey: String,
        closing_price: f32,
    ) -> Result<()> {
        let effected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(positions::position_state.eq(PositionState::Open))
            .set((
                positions::position_state.eq(PositionState::Closing),
                positions::closing_price.eq(Some(closing_price)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if effected_rows == 0 {
            bail!("Could not update position to Closing for {trader_pubkey}")
        }

        Ok(())
    }

    pub fn set_position_to_closed(conn: &mut PgConnection, id: i32, pnl: i64) -> Result<()> {
        let effected_rows = diesel::update(positions::table)
            .filter(positions::id.eq(id))
            .set((
                positions::position_state.eq(PositionState::Closed),
                positions::realized_pnl_sat.eq(Some(pnl)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if effected_rows == 0 {
            bail!("Could not update position to Closed with realized pnl {pnl} for position {id}")
        }

        Ok(())
    }

    pub fn update_unrealized_pnl(conn: &mut PgConnection, id: i32, pnl: i64) -> Result<()> {
        let effected_rows = diesel::update(positions::table)
            .filter(positions::id.eq(id))
            .set((
                positions::unrealized_pnl_sat.eq(Some(pnl)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if effected_rows == 0 {
            bail!("Could not update unrealized pnl {pnl} for position {id}")
        }

        Ok(())
    }

    /// inserts the given position into the db. Returns the position if successful
    #[autometrics]
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
            position_state: crate::position::models::PositionState::from((
                value.position_state,
                value.realized_pnl_sat,
            )),
            collateral: value.collateral,
            creation_timestamp: value.creation_timestamp,
            expiry_timestamp: value.expiry_timestamp,
            update_timestamp: value.update_timestamp,
            trader: value.trader_pubkey.parse().expect("to be valid public key"),
            temporary_contract_id: value.temporary_contract_id.map(|contract_id| {
                ContractId::from_hex(contract_id.as_str()).expect("contract id to decode")
            }),
            closing_price: value.closing_price,
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
    pub temporary_contract_id: String,
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
            temporary_contract_id: value.temporary_contract_id.to_hex(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = PositionStateType)]
pub enum PositionState {
    Open,
    Closing,
    Closed,
}

impl QueryId for PositionStateType {
    type QueryId = PositionStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl From<(PositionState, Option<i64>)> for crate::position::models::PositionState {
    fn from((position_state, realized_pnl): (PositionState, Option<i64>)) -> Self {
        match position_state {
            PositionState::Open => crate::position::models::PositionState::Open,
            PositionState::Closing => crate::position::models::PositionState::Closing,
            PositionState::Closed => crate::position::models::PositionState::Closed {
                pnl: realized_pnl.expect("realized pnl to be set when position is closed"),
            },
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
