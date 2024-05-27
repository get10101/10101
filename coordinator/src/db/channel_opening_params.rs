use crate::schema::channel_opening_params;
use bitcoin::Amount;
use diesel::ExpressionMethods;
use diesel::Insertable;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Queryable, QueryableByName, Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = channel_opening_params)]
pub struct ChannelOpeningParams {
    order_id: String,
    coordinator_reserve: i64,
    trader_reserve: i64,
    created_at: i64,
    external_funding: Option<i64>,
}

pub fn insert(
    conn: &mut PgConnection,
    order_id: Uuid,
    channel_opening_params: crate::ChannelOpeningParams,
) -> QueryResult<()> {
    let affected_rows = diesel::insert_into(channel_opening_params::table)
        .values(ChannelOpeningParams::from((
            order_id,
            channel_opening_params,
        )))
        .execute(conn)?;

    if affected_rows == 0 {
        return diesel::result::QueryResult::Err(diesel::result::Error::NotFound);
    }

    diesel::result::QueryResult::Ok(())
}

pub fn get_by_order_id(
    conn: &mut PgConnection,
    order_id: Uuid,
) -> QueryResult<Option<crate::ChannelOpeningParams>> {
    let channel_opening_params: Option<ChannelOpeningParams> = channel_opening_params::table
        .filter(channel_opening_params::order_id.eq(order_id.to_string()))
        .first(conn)
        .optional()?;

    Ok(channel_opening_params.map(crate::ChannelOpeningParams::from))
}

impl From<(Uuid, crate::ChannelOpeningParams)> for ChannelOpeningParams {
    fn from((order_id, channel_opening_params): (Uuid, crate::ChannelOpeningParams)) -> Self {
        Self {
            order_id: order_id.to_string(),
            trader_reserve: channel_opening_params.trader_reserve.to_sat() as i64,
            coordinator_reserve: channel_opening_params.coordinator_reserve.to_sat() as i64,
            external_funding: channel_opening_params
                .external_funding
                .map(|funding| funding.to_sat() as i64),
            created_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

impl From<ChannelOpeningParams> for crate::ChannelOpeningParams {
    fn from(value: ChannelOpeningParams) -> Self {
        Self {
            coordinator_reserve: Amount::from_sat(value.coordinator_reserve as u64),
            trader_reserve: Amount::from_sat(value.trader_reserve as u64),
            external_funding: value
                .external_funding
                .map(|funding| Amount::from_sat(funding as u64)),
        }
    }
}
