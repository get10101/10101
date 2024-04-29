use crate::db::positions::Position;
use crate::db::positions::PositionState;
use crate::decimal_from_f32;
use crate::f32_from_decimal;
use crate::funding_fee;
use crate::schema::funding_fee_events;
use crate::schema::positions;
use crate::schema::protocol_funding_fee_events;
use bitcoin::secp256k1::PublicKey;
use bitcoin::SignedAmount;
use diesel::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::OffsetDateTime;
use xxi_node::node::ProtocolId;

#[derive(Queryable, Debug)]
struct FundingFeeEvent {
    id: i32,
    /// A positive amount indicates that the trader pays the coordinator; a negative amount
    /// indicates that the coordinator pays the trader.
    amount_sats: i64,
    trader_pubkey: String,
    position_id: i32,
    due_date: OffsetDateTime,
    price: f32,
    funding_rate: f32,
    paid_date: Option<OffsetDateTime>,
    #[diesel(column_name = "timestamp")]
    _timestamp: OffsetDateTime,
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    amount: SignedAmount,
    trader_pubkey: PublicKey,
    position_id: i32,
    due_date: OffsetDateTime,
    price: Decimal,
    funding_rate: Decimal,
) -> QueryResult<Option<funding_fee::FundingFeeEvent>> {
    let res = diesel::insert_into(funding_fee_events::table)
        .values(&(
            funding_fee_events::amount_sats.eq(amount.to_sat()),
            funding_fee_events::trader_pubkey.eq(trader_pubkey.to_string()),
            funding_fee_events::position_id.eq(position_id),
            funding_fee_events::due_date.eq(due_date),
            funding_fee_events::price.eq(f32_from_decimal(price)),
            funding_fee_events::funding_rate.eq(f32_from_decimal(funding_rate)),
        ))
        .get_result::<FundingFeeEvent>(conn);

    match res {
        Ok(funding_fee_event) => Ok(Some(funding_fee::FundingFeeEvent::from(funding_fee_event))),
        Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        )) => {
            tracing::debug!(
                position_id,
                %trader_pubkey,
                %due_date,
                ?amount,
                "Funding fee event already exists in funding_fee_events table"
            );

            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Get all [`funding_fee::FundingFeeEvent`]s for the active positions of a given trader.
///
/// A trader may miss multiple funding fee events, particularly when they go offline. This function
/// allows us the coordinator to catch them up on reconnect.
///
/// # Returns
///
/// A list of [`xxi_node::FundingFeeEvent`]s, since these are to be sent to the trader via the
/// `xxi_node::Message::AllFundingFeeEvents` message.
pub(crate) fn get_for_active_trader_positions(
    conn: &mut PgConnection,
    trader_pubkey: PublicKey,
) -> QueryResult<Vec<xxi_node::FundingFeeEvent>> {
    let funding_fee_events: Vec<(FundingFeeEvent, Position)> = funding_fee_events::table
        .filter(funding_fee_events::trader_pubkey.eq(trader_pubkey.to_string()))
        .inner_join(positions::table.on(positions::id.eq(funding_fee_events::position_id)))
        .filter(
            positions::position_state
                .eq(PositionState::Open)
                .or(positions::position_state.eq(PositionState::Resizing))
                .or(positions::position_state.eq(PositionState::Rollover)),
        )
        .load(conn)?;

    let funding_fee_events = funding_fee_events
        .into_iter()
        .map(|(e, p)| xxi_node::FundingFeeEvent {
            contract_symbol: p.contract_symbol.into(),
            contracts: decimal_from_f32(p.quantity),
            direction: p.trader_direction.into(),
            price: decimal_from_f32(e.price),
            fee: SignedAmount::from_sat(e.amount_sats),
            due_date: e.due_date,
        })
        .collect();

    Ok(funding_fee_events)
}

/// Get the unpaid [`funding_fee::FundingFeeEvent`]s for a trader position.
///
/// TODO: Use outstanding fees when:
///
/// - Deciding if positions need to be liquidated.
/// - Closing a position.
/// - Resizing a position.
pub(crate) fn get_outstanding_fees(
    conn: &mut PgConnection,
    trader_pubkey: PublicKey,
    position_id: i32,
) -> QueryResult<Vec<funding_fee::FundingFeeEvent>> {
    let funding_events: Vec<FundingFeeEvent> = funding_fee_events::table
        .filter(
            funding_fee_events::trader_pubkey
                .eq(trader_pubkey.to_string())
                .and(funding_fee_events::position_id.eq(position_id))
                // If the `paid_date` is not set, the funding fee has not been paid.
                .and(funding_fee_events::paid_date.is_null()),
        )
        .load(conn)?;

    Ok(funding_events
        .iter()
        .map(funding_fee::FundingFeeEvent::from)
        .collect())
}

pub(crate) fn mark_as_paid(conn: &mut PgConnection, protocol_id: ProtocolId) -> QueryResult<()> {
    conn.transaction(|conn| {
        // Find all funding fee event IDs that were just paid.
        let funding_fee_event_ids: Vec<i32> = protocol_funding_fee_events::table
            .select(protocol_funding_fee_events::funding_fee_event_id)
            .filter(protocol_funding_fee_events::protocol_id.eq(protocol_id.to_uuid()))
            .load(conn)?;

        if funding_fee_event_ids.is_empty() {
            tracing::debug!(%protocol_id, "No funding fee events paid by protocol");

            return QueryResult::Ok(());
        }

        let now = OffsetDateTime::now_utc();

        // Mark funding fee events as paid.
        diesel::update(
            funding_fee_events::table.filter(funding_fee_events::id.eq_any(&funding_fee_event_ids)),
        )
        .set(funding_fee_events::paid_date.eq(now))
        .execute(conn)?;

        // Delete entries in `protocol_funding_fee_events` table.
        diesel::delete(
            protocol_funding_fee_events::table
                .filter(protocol_funding_fee_events::id.eq_any(&funding_fee_event_ids)),
        )
        .execute(conn)?;

        QueryResult::Ok(())
    })?;

    Ok(())
}

impl From<&FundingFeeEvent> for funding_fee::FundingFeeEvent {
    fn from(value: &FundingFeeEvent) -> Self {
        Self {
            id: value.id,
            amount: SignedAmount::from_sat(value.amount_sats),
            trader_pubkey: PublicKey::from_str(value.trader_pubkey.as_str())
                .expect("to be valid pk"),
            position_id: value.position_id,
            due_date: value.due_date,
            price: decimal_from_f32(value.price),
            funding_rate: decimal_from_f32(value.funding_rate),
            paid_date: value.paid_date,
        }
    }
}

impl From<FundingFeeEvent> for funding_fee::FundingFeeEvent {
    fn from(value: FundingFeeEvent) -> Self {
        Self {
            id: value.id,
            amount: SignedAmount::from_sat(value.amount_sats),
            trader_pubkey: PublicKey::from_str(value.trader_pubkey.as_str())
                .expect("to be valid pk"),
            position_id: value.position_id,
            due_date: value.due_date,
            price: decimal_from_f32(value.price),
            funding_rate: decimal_from_f32(value.funding_rate),
            paid_date: value.paid_date,
        }
    }
}
