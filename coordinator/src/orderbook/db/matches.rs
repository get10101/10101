use crate::orderbook::db::custom_types::MatchState;
use crate::orderbook::trading::TraderMatchParams;
use crate::schema::matches;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use diesel::ExpressionMethods;
use diesel::Insertable;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Insertable, QueryableByName, Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = matches)]
struct Matches {
    pub id: Uuid,
    pub match_state: MatchState,
    pub order_id: Uuid,
    pub trader_id: String,
    pub match_order_id: Uuid,
    pub match_trader_id: String,
    pub execution_price: f32,
    pub quantity: f32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub matching_fee_sats: i64,
}

pub fn insert(conn: &mut PgConnection, match_params: &TraderMatchParams) -> Result<()> {
    for record in Matches::new(match_params, MatchState::Pending) {
        let affected_rows = diesel::insert_into(matches::table)
            .values(record.clone())
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not insert matches");
    }

    Ok(())
}

pub fn set_match_state(
    conn: &mut PgConnection,
    order_id: Uuid,
    match_state: commons::MatchState,
) -> QueryResult<()> {
    diesel::update(matches::table)
        .filter(matches::order_id.eq(order_id))
        .set(matches::match_state.eq(MatchState::from(match_state)))
        .execute(conn)?;

    Ok(())
}

pub fn get_matches_by_order_id(
    conn: &mut PgConnection,
    order_id: Uuid,
) -> QueryResult<Vec<commons::Matches>> {
    let matches: Vec<Matches> = matches::table
        .filter(matches::order_id.eq(order_id))
        .load(conn)?;

    let matches = matches.into_iter().map(commons::Matches::from).collect();

    Ok(matches)
}

pub fn set_match_state_by_order_id(
    conn: &mut PgConnection,
    order_id: Uuid,
    match_state: commons::MatchState,
) -> Result<()> {
    let affected_rows = diesel::update(matches::table)
        .filter(matches::order_id.eq(order_id))
        .set(matches::match_state.eq(MatchState::from(match_state)))
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not update matches");
    Ok(())
}

impl Matches {
    pub fn new(match_params: &TraderMatchParams, match_state: MatchState) -> Vec<Matches> {
        let order_id = match_params.filled_with.order_id;
        let updated_at = OffsetDateTime::now_utc();
        let trader_id = match_params.trader_id;

        match_params
            .filled_with
            .matches
            .iter()
            .map(|m| Matches {
                id: m.id,
                match_state,
                order_id,
                trader_id: trader_id.to_string(),
                match_order_id: m.order_id,
                match_trader_id: m.pubkey.to_string(),
                execution_price: m.execution_price.to_f32().expect("to fit into f32"),
                quantity: m.quantity.to_f32().expect("to fit into f32"),
                created_at: updated_at,
                updated_at,
                matching_fee_sats: m.matching_fee.to_sat() as i64,
            })
            .collect()
    }
}

impl From<commons::Matches> for Matches {
    fn from(value: commons::Matches) -> Self {
        Matches {
            id: value.id,
            match_state: value.match_state.into(),
            order_id: value.order_id,
            trader_id: value.trader_id.to_string(),
            match_order_id: value.match_order_id,
            match_trader_id: value.match_trader_id.to_string(),
            execution_price: value.execution_price.to_f32().expect("to fit into f32"),
            quantity: value.quantity.to_f32().expect("to fit into f32"),
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            matching_fee_sats: value.matching_fee.to_sat() as i64,
        }
    }
}

impl From<commons::MatchState> for MatchState {
    fn from(value: commons::MatchState) -> Self {
        match value {
            commons::MatchState::Pending => MatchState::Pending,
            commons::MatchState::Filled => MatchState::Filled,
            commons::MatchState::Failed => MatchState::Failed,
        }
    }
}

impl From<Matches> for commons::Matches {
    fn from(value: Matches) -> Self {
        commons::Matches {
            id: value.id,
            match_state: value.match_state.into(),
            order_id: value.order_id,
            trader_id: PublicKey::from_str(&value.trader_id).expect("to be a valid public key"),
            match_order_id: value.match_order_id,
            match_trader_id: PublicKey::from_str(&value.match_trader_id)
                .expect("to be a valid public key"),
            execution_price: Decimal::from_f32(value.execution_price).expect("to fit into decimal"),
            quantity: Decimal::from_f32(value.quantity).expect("to fit into decimal"),
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            matching_fee: Amount::from_sat(value.matching_fee_sats as u64),
        }
    }
}

impl From<MatchState> for commons::MatchState {
    fn from(value: MatchState) -> Self {
        match value {
            MatchState::Pending => commons::MatchState::Pending,
            MatchState::Filled => commons::MatchState::Filled,
            MatchState::Failed => commons::MatchState::Failed,
        }
    }
}
