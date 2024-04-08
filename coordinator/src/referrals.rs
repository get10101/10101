use crate::db;
use crate::db::bonus_status::BonusType;
use crate::db::bonus_tiers::BonusTier;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::BonusStatusType;
use commons::ReferralStatus;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::OffsetDateTime;

/// When updating a referral status we only want to look at users which had a login in the last 48h.
const DAYS_SINCE_LAST_LOGIN: i64 = 2;

pub fn get_referral_status(
    trader_pubkey: PublicKey,
    connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<ReferralStatus> {
    let mut bonus_status = db::bonus_status::active_status_for_user(connection, &trader_pubkey)?;
    let user = db::user::get_user(connection, &trader_pubkey)?.context("User not found")?;

    // we sort by fee_rebate
    bonus_status.sort_by(|a, b| {
        b.fee_rebate
            .partial_cmp(&a.fee_rebate)
            .expect("to be able to sort")
    });

    // next we pick the highest
    if let Some(bonus) = bonus_status.first() {
        let referrals =
            db::bonus_tiers::all_referrals_by_referring_user(connection, &trader_pubkey)?;

        return Ok(ReferralStatus {
            referral_code: user.referral_code,
            number_of_activated_referrals: referrals
                .iter()
                .filter(|referral| referral.referred_user_total_quantity.floor() > 0.0)
                .count(),
            number_of_total_referrals: referrals.len(),
            referral_tier: bonus.tier_level as usize,
            referral_fee_bonus: Decimal::from_f32(bonus.fee_rebate).expect("to fit"),
            bonus_status_type: Some(bonus.bonus_type.into()),
        });
    }

    // None of the above, user is a boring normal user
    Ok(ReferralStatus::new(trader_pubkey))
}

pub fn update_referral_status(
    connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<usize> {
    let users = db::user::all_with_login_date(
        connection,
        OffsetDateTime::now_utc() - time::Duration::days(DAYS_SINCE_LAST_LOGIN),
    )?;
    let len = users.len();

    for user in users {
        let trader_pubkey = user.pubkey.clone();
        if let Err(err) = update_referral_status_for_user(connection, user.pubkey) {
            tracing::error!(
                trader_pubkey,
                "Failed at updating referral status for user {err}"
            )
        }
    }

    Ok(len)
}

/// Updates the referral status for a user based on data in the database
pub fn update_referral_status_for_user(
    connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
    trader_pubkey_str: String,
) -> Result<ReferralStatus> {
    let trader_pubkey =
        PublicKey::from_str(trader_pubkey_str.as_str()).expect("to be a valid pubkey");
    tracing::debug!(
        trader_pubkey = trader_pubkey_str,
        "Updating referral status"
    );

    // first we check his existing status. If he currently has an active referent status we return
    // here
    let status = get_referral_status(trader_pubkey, connection)?;
    if let Some(bonus_level) = &status.bonus_status_type {
        if BonusStatusType::Referent == *bonus_level {
            tracing::debug!(
                trader_pubkey = trader_pubkey_str,
                bonus_tier = status.referral_tier,
                bonus_level = ?bonus_level,
                "User has active bonus status"
            );
            return Ok(status);
        }
    }

    // next we need to calculate if he qualifier for a referral program
    let user = db::user::get_user(connection, &trader_pubkey)?.context("User not found")?;
    let referrals = db::bonus_tiers::all_referrals_by_referring_user(connection, &trader_pubkey)?;
    let bonus_tiers = db::bonus_tiers::all_active_by_type(connection, vec![BonusType::Referral])?;

    let total_referrals = referrals.len();

    let referral_code = user.referral_code;
    if total_referrals > 0 {
        let referral_tier = calculate_bonus_status_inner(bonus_tiers.clone(), total_referrals)?;
        let status = db::bonus_status::insert(
            connection,
            &trader_pubkey,
            referral_tier,
            BonusType::Referral,
        )?;
        tracing::debug!(
            trader_pubkey = trader_pubkey.to_string(),
            tier_level = status.tier_level,
            bonus_type = ?status.bonus_type,
            activation_timestamp = status.activation_timestamp.to_string(),
            deactivation_timestamp = status.deactivation_timestamp.to_string(),
            "Updated user's bonus status"
        );
        let maybe_bonus_tier = bonus_tiers
            .into_iter()
            .find(|tier| tier.tier_level == referral_tier)
            .context("Calculated bonus tier does not exist")?;

        tracing::debug!(
            trader_pubkey = trader_pubkey.to_string(),
            tier_level = maybe_bonus_tier.tier_level,
            bonus_tier_type = ?maybe_bonus_tier.bonus_tier_type,
            total_referrals = total_referrals,
            "Trader has referral status"
        );

        return Ok(ReferralStatus {
            referral_code,
            number_of_activated_referrals: referrals
                .iter()
                .filter(|referral| referral.referred_user_total_quantity.floor() > 0.0)
                .count(),
            number_of_total_referrals: total_referrals,
            referral_tier: maybe_bonus_tier.tier_level as usize,
            referral_fee_bonus: Decimal::from_f32(maybe_bonus_tier.fee_rebate).expect("to fit"),
            bonus_status_type: Some(maybe_bonus_tier.bonus_tier_type.into()),
        });
    }

    tracing::debug!(
        trader_pubkey = trader_pubkey.to_string(),
        "Trader doesn't have any new referral status yet"
    );

    // User doesn't have any new referral status yet
    Ok(status)
}

/// Returns the tier_level of the calculated tier.
///
/// e.g. user has 10 referrals, first 5 have already traded
/// bonus_tier_0 needs 3 referrals  
/// bonus_tier_1 needs 5 referrals
/// bonus_tier_2 needs 10 referrals
///
/// each referral only counts if they have traded at least once.
///
/// In this case, we should return tier 1.
fn calculate_bonus_status_inner(bonus_tiers: Vec<BonusTier>, referred_users: usize) -> Result<i32> {
    let mut bonus_tiers = bonus_tiers;

    // we sort descending by min referrals to have
    bonus_tiers.sort_by(|a, b| b.min_users_to_refer.cmp(&a.min_users_to_refer));

    if let Some(tier) = bonus_tiers
        .iter()
        .find(|tier| tier.min_users_to_refer <= referred_users as i32)
    {
        Ok(tier.tier_level)
    } else {
        Ok(0)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::db::bonus_status::BonusType;
    use crate::db::bonus_tiers::BonusTier;
    use crate::referrals::calculate_bonus_status_inner;

    #[test]
    pub fn given_no_referred_users_then_tier_level_0() {
        let referral_tier = calculate_bonus_status_inner(create_dummy_tiers(), 0).unwrap();

        assert_eq!(referral_tier, 0);
    }

    #[test]
    pub fn given_tier_1_referred_users_then_tier_level_1() {
        let referral_tier = calculate_bonus_status_inner(create_dummy_tiers(), 10).unwrap();

        assert_eq!(referral_tier, 1);
    }

    #[test]
    pub fn given_tier_2_referred_users_then_tier_level_2() {
        let referral_tier = calculate_bonus_status_inner(create_dummy_tiers(), 20).unwrap();

        assert_eq!(referral_tier, 2);
    }

    #[test]
    pub fn given_tier_1_and_not_enough_tier_2_referred_users_then_tier_level_1() {
        let referral_tier = calculate_bonus_status_inner(create_dummy_tiers(), 15).unwrap();

        assert_eq!(referral_tier, 1);
    }

    #[test]
    pub fn given_enough_tier_3_referred_users_then_tier_level_3() {
        let referral_tier = calculate_bonus_status_inner(create_dummy_tiers(), 30).unwrap();

        assert_eq!(referral_tier, 3);
    }

    fn create_dummy_tiers() -> Vec<BonusTier> {
        vec![
            BonusTier {
                id: 0,
                tier_level: 0,
                min_users_to_refer: 0,
                fee_rebate: 0.0,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
            BonusTier {
                id: 1,
                tier_level: 1,
                min_users_to_refer: 10,
                fee_rebate: 0.2,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
            BonusTier {
                id: 2,
                tier_level: 2,
                min_users_to_refer: 20,
                fee_rebate: 0.3,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
            BonusTier {
                id: 3,
                tier_level: 3,
                min_users_to_refer: 30,
                fee_rebate: 0.3,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
        ]
    }
}
