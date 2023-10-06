use crate::schema::liquidity_request_logs;
use bitcoin::secp256k1::PublicKey;
use diesel::prelude::*;
use time::OffsetDateTime;

#[derive(Insertable, Queryable, Identifiable, AsChangeset)]
pub struct LiquidityRequestLog {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub trader_pk: String,
    pub timestamp: OffsetDateTime,
    pub requested_amount_sats: i64,
    pub liquidity_option: i32,
    pub successfully_requested: bool,
}

impl LiquidityRequestLog {
    pub fn insert(
        conn: &mut PgConnection,
        trader_pk: PublicKey,
        requested_amount_sats: u64,
        liquidity_option: i32,
        successfully_requested: bool,
    ) -> QueryResult<Self> {
        let liquidity_request_log = LiquidityRequestLog {
            id: None,
            trader_pk: trader_pk.to_string(),
            timestamp: OffsetDateTime::now_utc(),
            requested_amount_sats: requested_amount_sats as i64,
            liquidity_option,
            successfully_requested,
        };

        diesel::insert_into(liquidity_request_logs::table)
            .values(liquidity_request_log)
            .get_result(conn)
    }
}
