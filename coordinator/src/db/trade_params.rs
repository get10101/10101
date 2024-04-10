use crate::dlc_protocol;
use crate::dlc_protocol::ProtocolId;
use crate::orderbook::db::custom_types::Direction;
use crate::schema::trade_params;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::SignedAmount;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Queryable, Debug)]
#[diesel(table_name = trade_params)]
#[allow(dead_code)] // We have to allow dead code here because diesel needs the fields to be able to derive queryable.
pub(crate) struct TradeParams {
    pub id: i32,
    pub protocol_id: Uuid,
    pub trader_pubkey: String,
    pub quantity: f32,
    pub leverage: f32,
    pub average_price: f32,
    pub direction: Direction,
    pub matching_fee: i64,
    pub trader_pnl: Option<i64>,
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
    params: &dlc_protocol::TradeParams,
) -> QueryResult<()> {
    let affected_rows = diesel::insert_into(trade_params::table)
        .values(&(
            trade_params::protocol_id.eq(protocol_id.to_uuid()),
            trade_params::quantity.eq(params.quantity),
            trade_params::leverage.eq(params.leverage),
            trade_params::trader_pubkey.eq(params.trader.to_string()),
            trade_params::direction.eq(Direction::from(params.direction)),
            trade_params::average_price.eq(params.average_price),
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
) -> QueryResult<dlc_protocol::TradeParams> {
    let trade_params: TradeParams = trade_params::table
        .filter(trade_params::protocol_id.eq(protocol_id.to_uuid()))
        .first(conn)?;

    Ok(dlc_protocol::TradeParams::from(trade_params))
}

impl From<TradeParams> for dlc_protocol::TradeParams {
    fn from(value: TradeParams) -> Self {
        Self {
            protocol_id: value.protocol_id.into(),
            trader: PublicKey::from_str(&value.trader_pubkey).expect("valid pubkey"),
            quantity: value.quantity,
            leverage: value.leverage,
            average_price: value.average_price,
            direction: trade::Direction::from(value.direction),
            matching_fee: Amount::from_sat(value.matching_fee as u64),
            trader_pnl: value.trader_pnl.map(SignedAmount::from_sat),
        }
    }
}
