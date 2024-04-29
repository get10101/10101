use crate::dlc_protocol;
use crate::schema::rollover_params;
use bitcoin::Amount;
use diesel::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;
use xxi_node::node::ProtocolId;

#[derive(Queryable, Debug)]
#[diesel(table_name = rollover_params)]
struct RolloverParams {
    #[diesel(column_name = "id")]
    _id: i32,
    protocol_id: Uuid,
    trader_pubkey: String,
    margin_coordinator_sat: i64,
    margin_trader_sat: i64,
    leverage_coordinator: f32,
    leverage_trader: f32,
    liquidation_price_coordinator: f32,
    liquidation_price_trader: f32,
    expiry_timestamp: OffsetDateTime,
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    params: &dlc_protocol::RolloverParams,
) -> QueryResult<()> {
    let dlc_protocol::RolloverParams {
        protocol_id,
        trader_pubkey,
        margin_coordinator,
        margin_trader,
        leverage_coordinator,
        leverage_trader,
        liquidation_price_coordinator,
        liquidation_price_trader,
        expiry_timestamp,
    } = params;

    let affected_rows = diesel::insert_into(rollover_params::table)
        .values(&(
            rollover_params::protocol_id.eq(protocol_id.to_uuid()),
            rollover_params::trader_pubkey.eq(trader_pubkey.to_string()),
            rollover_params::margin_coordinator_sat.eq(margin_coordinator.to_sat() as i64),
            rollover_params::margin_trader_sat.eq(margin_trader.to_sat() as i64),
            rollover_params::leverage_coordinator
                .eq(leverage_coordinator.to_f32().expect("to fit")),
            rollover_params::leverage_trader.eq(leverage_trader.to_f32().expect("to fit")),
            rollover_params::liquidation_price_coordinator
                .eq(liquidation_price_coordinator.to_f32().expect("to fit")),
            rollover_params::liquidation_price_trader
                .eq(liquidation_price_trader.to_f32().expect("to fit")),
            rollover_params::expiry_timestamp.eq(expiry_timestamp),
        ))
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}

pub(crate) fn get(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
) -> QueryResult<dlc_protocol::RolloverParams> {
    let RolloverParams {
        _id,
        trader_pubkey,
        protocol_id,
        margin_coordinator_sat: margin_coordinator,
        margin_trader_sat: margin_trader,
        leverage_coordinator,
        leverage_trader,
        liquidation_price_coordinator,
        liquidation_price_trader,
        expiry_timestamp,
    } = rollover_params::table
        .filter(rollover_params::protocol_id.eq(protocol_id.to_uuid()))
        .first(conn)?;

    Ok(dlc_protocol::RolloverParams {
        protocol_id: protocol_id.into(),
        trader_pubkey: trader_pubkey.parse().expect("valid pubkey"),
        margin_coordinator: Amount::from_sat(margin_coordinator as u64),
        margin_trader: Amount::from_sat(margin_trader as u64),
        leverage_coordinator: Decimal::try_from(leverage_coordinator).expect("to fit"),
        leverage_trader: Decimal::try_from(leverage_trader).expect("to fit"),
        liquidation_price_coordinator: Decimal::try_from(liquidation_price_coordinator)
            .expect("to fit"),
        liquidation_price_trader: Decimal::try_from(liquidation_price_trader).expect("to fit"),
        expiry_timestamp,
    })
}
