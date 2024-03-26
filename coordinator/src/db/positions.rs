use crate::orderbook::db::custom_types::Direction;
use crate::schema::positions;
use crate::schema::sql_types::ContractSymbolType;
use crate::schema::sql_types::PositionStateType;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
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
    pub trader_leverage: f32,
    pub quantity: f32,
    pub trader_direction: Direction,
    pub average_entry_price: f32,
    pub trader_liquidation_price: f32,
    pub position_state: PositionState,
    pub coordinator_margin: i64,
    pub creation_timestamp: OffsetDateTime,
    pub expiry_timestamp: OffsetDateTime,
    pub update_timestamp: OffsetDateTime,
    pub trader_pubkey: String,
    pub temporary_contract_id: Option<String>,
    pub trader_realized_pnl_sat: Option<i64>,
    pub trader_unrealized_pnl_sat: Option<i64>,
    pub closing_price: Option<f32>,
    pub coordinator_leverage: f32,
    pub trader_margin: i64,
    pub stable: bool,
}

impl Position {
    /// Returns the position by trader pub key
    pub fn get_position_by_trader(
        conn: &mut PgConnection,
        trader_pubkey: PublicKey,
        states: Vec<crate::position::models::PositionState>,
    ) -> QueryResult<Option<crate::position::models::Position>> {
        let mut query = positions::table.into_boxed();

        query = query.filter(positions::trader_pubkey.eq(trader_pubkey.to_string()));

        if !states.is_empty() {
            query = query.filter(
                positions::position_state.eq_any(states.into_iter().map(PositionState::from)),
            )
        }

        let x = query
            .order_by(positions::creation_timestamp.desc())
            .first::<Position>(conn)
            .optional()?;

        Ok(x.map(crate::position::models::Position::from))
    }

    pub fn get_all_open_positions_with_expiry_before(
        conn: &mut PgConnection,
        expiry: OffsetDateTime,
    ) -> QueryResult<Vec<crate::position::models::Position>> {
        let positions = positions::table
            .filter(positions::position_state.eq(PositionState::Open))
            .filter(positions::expiry_timestamp.lt(expiry))
            .load::<Position>(conn)?;

        let positions = positions
            .into_iter()
            .map(crate::position::models::Position::from)
            .collect();

        Ok(positions)
    }

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

    pub fn get_all_closed_positions(
        conn: &mut PgConnection,
    ) -> QueryResult<Vec<crate::position::models::Position>> {
        let positions = positions::table
            .filter(positions::position_state.eq(PositionState::Closed))
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

    /// sets the status of the position in state `Proposed` to a new state
    pub fn update_proposed_position(
        conn: &mut PgConnection,
        trader_pubkey: String,
        state: crate::position::models::PositionState,
    ) -> QueryResult<crate::position::models::Position> {
        let state = PositionState::from(state);
        let position: Position = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(
                positions::position_state
                    .eq(PositionState::Proposed)
                    .or(positions::position_state.eq(PositionState::ResizeProposed)),
            )
            .set((
                positions::position_state.eq(state),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .get_result(conn)?;

        Ok(crate::position::models::Position::from(position))
    }

    /// sets the status of the position in state `Closing` to a new state
    pub fn update_closing_position(
        conn: &mut PgConnection,
        trader_pubkey: String,
        state: crate::position::models::PositionState,
    ) -> Result<()> {
        let state = PositionState::from(state);
        let affected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(positions::position_state.eq(PositionState::Closing))
            .set((
                positions::position_state.eq(state),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position to {state:?} for {trader_pubkey}")
        }

        Ok(())
    }

    /// sets the status of all open position to closing (note, we expect that number to be always
    /// exactly 1)
    pub fn set_open_position_to_closing(
        conn: &mut PgConnection,
        trader_pubkey: String,
        closing_price: f32,
    ) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(positions::position_state.eq(PositionState::Open))
            .set((
                positions::position_state.eq(PositionState::Closing),
                positions::closing_price.eq(Some(closing_price)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position to Closing for {trader_pubkey}")
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_open_position_to_resizing(
        conn: &mut PgConnection,
        trader_pubkey: String,
    ) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(positions::position_state.eq(PositionState::Open))
            .set((
                positions::position_state.eq(PositionState::Resizing),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position to Resizing for {trader_pubkey}")
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_resized_position(
        conn: &mut PgConnection,
        trader_pubkey: String,
        quantity: f32,
        direction: Direction,
        coordinator_leverage: f32,
        trader_leverage: f32,
        coordinator_margin: i64,
        trader_margin: i64,
        average_entry_price: f32,
        liquidation_price: f32,
        expiry_timestamp: OffsetDateTime,
        temporary_contract_id: ContractId,
    ) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey.clone()))
            .filter(positions::position_state.eq(PositionState::Resizing))
            .set((
                positions::position_state.eq(PositionState::ResizeProposed),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
                positions::quantity.eq(quantity),
                positions::trader_direction.eq(direction),
                positions::trader_leverage.eq(trader_leverage),
                positions::coordinator_leverage.eq(coordinator_leverage),
                positions::coordinator_margin.eq(coordinator_margin),
                positions::trader_margin.eq(trader_margin),
                positions::average_entry_price.eq(average_entry_price),
                positions::trader_liquidation_price.eq(liquidation_price),
                positions::expiry_timestamp.eq(expiry_timestamp),
                positions::temporary_contract_id.eq(hex::encode(temporary_contract_id)),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position to Resizing for {trader_pubkey}")
        }

        Ok(())
    }

    pub fn set_position_to_closed_with_pnl(
        conn: &mut PgConnection,
        id: i32,
        trader_realized_pnl_sat: i64,
    ) -> QueryResult<crate::position::models::Position> {
        let position: Position = diesel::update(positions::table)
            .filter(positions::id.eq(id))
            .set((
                positions::position_state.eq(PositionState::Closed),
                positions::trader_realized_pnl_sat.eq(Some(trader_realized_pnl_sat)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .get_result(conn)?;

        Ok(crate::position::models::Position::from(position))
    }

    pub fn set_position_to_closed(conn: &mut PgConnection, id: i32) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(positions::id.eq(id))
            .set((
                positions::position_state.eq(PositionState::Closed),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position to Closed for position {id}")
        }

        Ok(())
    }

    pub fn set_position_to_open(
        conn: &mut PgConnection,
        trader_pubkey: String,
        temporary_contract_id: ContractId,
    ) -> QueryResult<usize> {
        diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey))
            .filter(
                positions::position_state
                    .eq(PositionState::Rollover)
                    .or(positions::position_state.eq(PositionState::Resizing)),
            )
            .set((
                positions::position_state.eq(PositionState::Open),
                positions::temporary_contract_id.eq(hex::encode(temporary_contract_id)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)
    }

    pub fn update_unrealized_pnl(conn: &mut PgConnection, id: i32, pnl: i64) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(positions::id.eq(id))
            .set((
                positions::trader_unrealized_pnl_sat.eq(Some(pnl)),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update unrealized pnl {pnl} for position {id}")
        }

        Ok(())
    }

    pub fn rollover_position(
        conn: &mut PgConnection,
        trader_pubkey: String,
        expiry_timestamp: &OffsetDateTime,
    ) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(positions::trader_pubkey.eq(trader_pubkey))
            .filter(positions::position_state.eq(PositionState::Open))
            .set((
                positions::expiry_timestamp.eq(expiry_timestamp),
                positions::position_state.eq(PositionState::Rollover),
                positions::update_timestamp.eq(OffsetDateTime::now_utc()),
            ))
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not set position to rollover");

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

impl From<crate::position::models::PositionState> for PositionState {
    fn from(value: crate::position::models::PositionState) -> Self {
        match value {
            crate::position::models::PositionState::Open => PositionState::Open,
            crate::position::models::PositionState::Closing { .. } => PositionState::Closing,
            crate::position::models::PositionState::Closed { .. } => PositionState::Closed,
            crate::position::models::PositionState::Rollover => PositionState::Rollover,
            crate::position::models::PositionState::Resizing { .. } => PositionState::Resizing,
            crate::position::models::PositionState::ResizeOpeningSubchannelProposed => {
                PositionState::ResizeProposed
            }
            crate::position::models::PositionState::Proposed => PositionState::Proposed,
            crate::position::models::PositionState::Failed => PositionState::Failed,
        }
    }
}

impl From<Position> for crate::position::models::Position {
    fn from(value: Position) -> Self {
        crate::position::models::Position {
            id: value.id,
            contract_symbol: trade::ContractSymbol::from(value.contract_symbol),
            trader_leverage: value.trader_leverage,
            quantity: value.quantity,
            trader_direction: trade::Direction::from(value.trader_direction),
            average_entry_price: value.average_entry_price,
            trader_liquidation_price: value.trader_liquidation_price,
            position_state: crate::position::models::PositionState::from((
                value.position_state,
                value.trader_realized_pnl_sat,
                value.closing_price,
            )),
            coordinator_margin: value.coordinator_margin,
            creation_timestamp: value.creation_timestamp,
            expiry_timestamp: value.expiry_timestamp,
            update_timestamp: value.update_timestamp,
            trader: value.trader_pubkey.parse().expect("to be valid public key"),
            temporary_contract_id: value.temporary_contract_id.map(|contract_id| {
                ContractId::from_hex(contract_id.as_str()).expect("contract id to decode")
            }),
            closing_price: value.closing_price,
            coordinator_leverage: value.coordinator_leverage,
            trader_margin: value.trader_margin,
            stable: value.stable,
            trader_realized_pnl_sat: value.trader_realized_pnl_sat,
        }
    }
}

#[derive(Insertable, Debug, PartialEq)]
#[diesel(table_name = positions)]
struct NewPosition {
    pub contract_symbol: ContractSymbol,
    pub trader_leverage: f32,
    pub quantity: f32,
    pub trader_direction: Direction,
    pub average_entry_price: f32,
    pub trader_liquidation_price: f32,
    pub position_state: PositionState,
    pub coordinator_margin: i64,
    pub expiry_timestamp: OffsetDateTime,
    pub trader_pubkey: String,
    pub temporary_contract_id: String,
    pub coordinator_leverage: f32,
    pub trader_margin: i64,
    pub stable: bool,
}

impl From<crate::position::models::NewPosition> for NewPosition {
    fn from(value: crate::position::models::NewPosition) -> Self {
        NewPosition {
            contract_symbol: ContractSymbol::from(value.contract_symbol),
            trader_leverage: value.trader_leverage,
            quantity: value.quantity,
            trader_direction: Direction::from(value.trader_direction),
            average_entry_price: value.average_entry_price,
            trader_liquidation_price: value.trader_liquidation_price,
            position_state: PositionState::Proposed,
            coordinator_margin: value.coordinator_margin,
            expiry_timestamp: value.expiry_timestamp,
            trader_pubkey: value.trader.to_string(),
            temporary_contract_id: hex::encode(value.temporary_contract_id),
            coordinator_leverage: value.coordinator_leverage,
            trader_margin: value.trader_margin,
            stable: value.stable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = PositionStateType)]
pub enum PositionState {
    Proposed,
    Open,
    Closing,
    Rollover,
    Closed,
    Failed,
    Resizing,
    ResizeProposed,
}

impl QueryId for PositionStateType {
    type QueryId = PositionStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl From<(PositionState, Option<i64>, Option<f32>)> for crate::position::models::PositionState {
    fn from(
        (position_state, realized_pnl, closing_price): (PositionState, Option<i64>, Option<f32>),
    ) -> Self {
        match position_state {
            PositionState::Open => crate::position::models::PositionState::Open,
            PositionState::Closing => crate::position::models::PositionState::Closing {
                // For backwards compatibility we set the closing price to 0 if it was not set in
                // `Closing` state
                closing_price: closing_price.unwrap_or(0.0_f32),
            },
            PositionState::Closed => crate::position::models::PositionState::Closed {
                // For backwards compatibility we set the realized pnl to 0 if it was not set in
                // `Closed` state
                pnl: realized_pnl.unwrap_or(0),
            },
            PositionState::Rollover => crate::position::models::PositionState::Rollover,
            PositionState::Resizing => crate::position::models::PositionState::Resizing,
            PositionState::Proposed => crate::position::models::PositionState::Proposed,
            PositionState::Failed => crate::position::models::PositionState::Failed,
            PositionState::ResizeProposed => {
                crate::position::models::PositionState::ResizeOpeningSubchannelProposed
            }
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
