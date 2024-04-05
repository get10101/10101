use crate::db;
use crate::db::bonus_status::BonusType;
use crate::schema::bonus_tiers;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Float;
use diesel::sql_types::Text;
use diesel::sql_types::Timestamptz;
use diesel::PgConnection;
use diesel::QueryResult;
use diesel::Queryable;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::OffsetDateTime;

pub struct Referral {
    pub volume: Decimal,
}

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = bonus_tiers)]
pub(crate) struct BonusTier {
    // this is needed because this field needs to be here to satisfy diesel
    #[allow(dead_code)]
    pub(crate) id: i32,
    pub(crate) tier_level: i32,
    pub(crate) min_users_to_refer: i32,
    pub(crate) min_volume_per_referral: i32,
    // TODO: to be used
    #[allow(dead_code)]
    pub(crate) fee_rebate: f32,
    #[allow(dead_code)]
    pub(crate) number_of_trades: i32,
    pub(crate) bonus_tier_type: BonusType,
    // this is needed because this field needs to be here to satisfy diesel
    #[allow(dead_code)]
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

/// Returns volume of referred users
pub fn get_referrals_per_referral_code(
    connection: &mut PgConnection,
    referral_code: String,
) -> Result<Vec<Referral>> {
    let referred_users = db::user::get_referred_users(connection, referral_code)?;

    let mut referrals = vec![];
    for user in referred_users {
        let public_key = PublicKey::from_str(user.pubkey.as_str()).expect("To be a valid pubkey");
        let trades = db::trades::get_trades(connection, public_key)?;
        let total_volume = trades.iter().map(|trade| trade.quantity).sum::<f32>();
        tracing::debug!(
            referred_user_id = user.pubkey,
            total_volume,
            "Referred user found"
        );
        referrals.push(Referral {
            volume: Decimal::from_f32(total_volume).expect("to fit into f32"),
        })
    }

    Ok(referrals)
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
                    FROM user_referral_summary_view where referring_user = $1";
    let summaries: Vec<UserReferralSummaryView> = sql_query(query)
        .bind::<Text, _>(trader_pubkey.to_string())
        .load(conn)?;

    Ok(summaries)
}
