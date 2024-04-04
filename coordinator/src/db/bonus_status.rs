use crate::db::bonus_tiers;
use crate::db::referral_tiers::tier_by_tier_level;
use crate::schema::bonus_status;
use bitcoin::secp256k1::PublicKey;
use diesel::ExpressionMethods;
use diesel::Insertable;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use time::OffsetDateTime;

/// A user's referral bonus status may be active for this much days at max
const MAX_DAYS_FOR_ACTIVE_REFERRAL_STATUS: i64 = 30;

#[allow(dead_code)]
// this is needed because the fields needs to be here to satisfy diesel
#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = bonus_status)]
pub(crate) struct BonusStatus {
    pub(crate) id: i32,
    pub(crate) trader_pubkey: String,
    pub(crate) tier_level: i32,
    pub(crate) fee_rebate: f32,
    pub(crate) remaining_trades: i32,
    pub(crate) activation_timestamp: OffsetDateTime,
    pub(crate) deactivation_timestamp: OffsetDateTime,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = bonus_status)]
pub(crate) struct NewBonusStatus {
    pub(crate) trader_pubkey: String,
    pub(crate) tier_level: i32,
    pub(crate) fee_rebate: f32,
    pub(crate) remaining_trades: i32,
    pub(crate) activation_timestamp: OffsetDateTime,
    pub(crate) deactivation_timestamp: OffsetDateTime,
}

/// This function might return multiple status for a single user
///
/// Because he might have moved up into the next level without the old level being expired. The
/// caller is responsible in picking the most suitable status
pub(crate) fn active_status_for_user(
    conn: &mut PgConnection,
    trader_pubkey: &PublicKey,
) -> QueryResult<Vec<BonusStatus>> {
    bonus_status::table
        .filter(bonus_status::trader_pubkey.eq(trader_pubkey.to_string()))
        .filter(bonus_status::deactivation_timestamp.gt(OffsetDateTime::now_utc()))
        .load(conn)
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    trader_pk: &PublicKey,
    tier_level: i32,
) -> QueryResult<BonusStatus> {
    let tier = tier_by_tier_level(conn, tier_level)?;
    let existing_status_for_user = active_status_for_user(conn, trader_pk)?;
    let bonus_tier = bonus_tiers::tier_by_tier_level(conn, tier_level)?;

    if let Some(status) = existing_status_for_user
        .into_iter()
        .find(|status| status.tier_level == tier_level)
    {
        tracing::debug!(
            trader_pubkey = trader_pk.to_string(),
            tier_level,
            "User has already gained bonus status"
        );
        return Ok(status);
    }

    let bonus_status = diesel::insert_into(bonus_status::table)
        .values(NewBonusStatus {
            trader_pubkey: trader_pk.to_string(),
            tier_level,
            fee_rebate: bonus_tier.fee_rebate,
            remaining_trades: tier.number_of_trades,
            activation_timestamp: OffsetDateTime::now_utc(),
            deactivation_timestamp: OffsetDateTime::now_utc()
                + time::Duration::days(MAX_DAYS_FOR_ACTIVE_REFERRAL_STATUS),
        })
        .get_result(conn)?;

    Ok(bonus_status)
}
