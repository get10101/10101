use crate::db::bonus_status::BonusType;
use crate::schema::bonus_tiers;
use bitcoin::secp256k1::PublicKey;
use diesel::pg::sql_types::Timestamptz;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Float;
use diesel::sql_types::Text;
use diesel::PgConnection;
use diesel::QueryResult;
use diesel::Queryable;
use rust_decimal::Decimal;
use time::OffsetDateTime;

pub struct Referral {
    pub volume: Decimal,
}

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = bonus_tiers)]
// this is needed because some fields are unused but need to be here for diesel
#[allow(dead_code)]
pub(crate) struct BonusTier {
    pub(crate) id: i32,
    pub(crate) tier_level: i32,
    pub(crate) min_users_to_refer: i32,
    pub(crate) fee_rebate: f32,
    pub(crate) bonus_tier_type: BonusType,
    pub(crate) active: bool,
}

/// Returns all active bonus tiers for given types
pub(crate) fn all_active_by_type(
    conn: &mut PgConnection,
    types: Vec<BonusType>,
) -> QueryResult<Vec<BonusTier>> {
    bonus_tiers::table
        .filter(bonus_tiers::active.eq(true))
        .filter(bonus_tiers::bonus_tier_type.eq_any(types))
        .load::<BonusTier>(conn)
}

pub(crate) fn tier_by_tier_level(
    conn: &mut PgConnection,
    tier_level: i32,
) -> QueryResult<BonusTier> {
    bonus_tiers::table
        .filter(bonus_tiers::tier_level.eq(tier_level))
        .first(conn)
}

#[derive(Debug, QueryableByName, Clone)]
pub struct UserReferralSummaryView {
    #[diesel(sql_type = Text)]
    pub referring_user: String,
    #[diesel(sql_type = Text)]
    pub referring_user_referral_code: String,
    #[diesel(sql_type = Text)]
    pub referred_user: String,
    #[diesel(sql_type = Text)]
    pub referred_user_referral_code: String,
    #[diesel(sql_type = Timestamptz)]
    pub timestamp: OffsetDateTime,
    #[diesel(sql_type = Float)]
    pub referred_user_total_quantity: f32,
}

/// Returns all referred users for by referrer with trading volume > 0
pub(crate) fn all_referrals_by_referring_user(
    conn: &mut PgConnection,
    trader_pubkey: &PublicKey,
) -> QueryResult<Vec<UserReferralSummaryView>> {
    // we have to do this manually because diesel does not support views. If you make a change to
    // below, make sure to test this against a life db as errors will only be thrown at runtime
    let query = "SELECT referring_user, referring_user_referral_code, \
                referred_user, \
                referred_user_referral_code, \
                timestamp, \
                referred_user_total_quantity \
                    FROM user_referral_summary_view where referring_user = $1 \
                    and referred_user_total_quantity > 0";
    let summaries: Vec<UserReferralSummaryView> = sql_query(query)
        .bind::<Text, _>(trader_pubkey.to_string())
        .load(conn)?;

    Ok(summaries)
}
