use crate::db;
use crate::db::referral_tiers::ReferralTier;
use crate::db::referral_tiers::UserReferralSummaryView;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::referral_from_pubkey;
use commons::ReferralStatus;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use time::OffsetDateTime;

/// When updating a referral status we only want to look at users which had a login in the last 48h.
const DAYS_SINCE_LAST_LOGIN: i64 = 2;

pub fn get_referral_status(
    trader_pubkey: PublicKey,
    connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<ReferralStatus> {
    let user = db::user::get_user(connection, &trader_pubkey)?.context("User not found")?;
    let referrals =
        db::referral_tiers::all_referrals_by_referring_user(connection, &trader_pubkey)?;
    let referral_tiers = db::referral_tiers::all_active(connection)?;

    let referral_code = user.referral_code;

    let mut status = db::bonus_status::active_status_for_user(connection, &trader_pubkey)?;
    if status.is_empty() {
        return Ok(ReferralStatus::new(trader_pubkey));
    }

    // we take the highest tier
    status.sort_by(|a, b| b.tier_level.cmp(&a.tier_level));

    let bonus_status = status
        .first()
        .expect("to have at least 1 element due to the check above");

    if bonus_status.remaining_trades <= 0
        || bonus_status
            .deactivation_timestamp
            .le(&OffsetDateTime::now_utc())
    {
        tracing::debug!(
            trader_pubkey = user.pubkey,
            tier_level = bonus_status.tier_level,
            "User's bonus status has already been expired"
        );
        return Ok(ReferralStatus::new(trader_pubkey));
    }

    let referral_tier = referral_tiers
        .iter()
        .find(|tier| tier.tier_level == bonus_status.tier_level)
        .context("User's referral tier not found")?;

    Ok(ReferralStatus {
        referral_code,
        number_of_activated_referrals: referrals
            .iter()
            .filter(|referral| {
                referral.referred_user_total_quantity.floor() as i32
                    > referral_tier.min_volume_per_referral
            })
            .count(),
        number_of_total_referrals: referrals.len(),
        referral_tier: bonus_status.tier_level as usize,
        referral_fee_bonus: Decimal::from_f32(referral_tier.fee_rebate).expect("to fit"),
    })
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
    trader_pubkey: String,
) -> Result<ReferralStatus> {
    let trader_pubkey = PublicKey::from_str(trader_pubkey.as_str()).expect("to be a valid pubkey");
    let referrals =
        db::referral_tiers::all_referrals_by_referring_user(connection, &trader_pubkey)?;
    let referral_tiers = db::referral_tiers::all_active(connection)?;
    let referral_status = calculate_referral_tier_inner(
        referrals,
        referral_tiers,
        referral_from_pubkey(trader_pubkey),
    )?;
    let status = db::bonus_status::insert(
        connection,
        &trader_pubkey,
        referral_status.referral_tier as i32,
    )?;
    tracing::debug!(
        trader_pubkey = trader_pubkey.to_string(),
        tier_level = status.tier_level,
        remaining_trades = status.remaining_trades,
        activation_timestamp = status.activation_timestamp.to_string(),
        deactivation_timestamp = status.deactivation_timestamp.to_string(),
        "Updated user's bonus status"
    );
    Ok(referral_status)
}

fn calculate_referral_tier_inner(
    referrals: Vec<UserReferralSummaryView>,
    referral_tiers: Vec<ReferralTier>,
    referral_code: String,
) -> Result<ReferralStatus> {
    let mut referred_users_sorted_by_tier: HashMap<i32, i32> = HashMap::new();

    let mut referral_tiers = referral_tiers;

    // we sort descending by volume so that we can pick the highest suitable tier below
    referral_tiers.sort_by(|a, b| b.min_volume_per_referral.cmp(&a.min_volume_per_referral));

    for referred_user in referrals {
        let volume = referred_user.referred_user_total_quantity;
        if let Some(tier) = referral_tiers
            .iter()
            .find(|tier| volume.to_i32().expect("to fit into i32") >= tier.min_volume_per_referral)
        {
            referred_users_sorted_by_tier.insert(
                tier.tier_level,
                referred_users_sorted_by_tier
                    .get(&tier.tier_level)
                    .cloned()
                    .unwrap_or_default()
                    + 1,
            );
        }
    }

    let mut selected_tier = None;
    // next we check if we have reached a tier level
    for tier in referral_tiers {
        if let Some(number_of_users) = referred_users_sorted_by_tier.get(&tier.tier_level) {
            if *number_of_users >= tier.min_users_to_refer {
                selected_tier.replace(tier);
                break;
            }
        }
    }

    let mut number_of_activated_referrals = 0;
    if let Some(ref selected_tier) = selected_tier {
        number_of_activated_referrals = referred_users_sorted_by_tier
            .get(&selected_tier.tier_level)
            .cloned()
            .unwrap_or_default() as usize
    }

    Ok(ReferralStatus {
        referral_code,
        number_of_activated_referrals,
        number_of_total_referrals: referred_users_sorted_by_tier.values().sum::<i32>() as usize,
        referral_tier: selected_tier
            .clone()
            .map(|t| t.tier_level)
            .unwrap_or_default() as usize,
        referral_fee_bonus: Decimal::from_f32(
            selected_tier
                .clone()
                .map(|t| t.fee_rebate)
                .unwrap_or_default(),
        )
        .expect("to be able to parse"),
    })
}

#[cfg(test)]
pub mod tests {
    use crate::db::referral_tiers::ReferralTier;
    use crate::db::referral_tiers::UserReferralSummaryView;
    use crate::referrals::calculate_referral_tier_inner;
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::OffsetDateTime;

    #[test]
    pub fn given_no_referred_users_then_tier_level_0() {
        let referral_code = "DUMMY".to_string();
        let referral_tier =
            calculate_referral_tier_inner(vec![], create_dummy_tiers(), referral_code.clone())
                .unwrap();

        assert_eq!(referral_tier.referral_tier, 0);
        assert_eq!(referral_tier.referral_fee_bonus, Decimal::ZERO);
        assert_eq!(referral_tier.referral_code, referral_code);
        assert_eq!(referral_tier.number_of_activated_referrals, 0);
        assert_eq!(referral_tier.number_of_total_referrals, 0);
    }

    #[test]
    pub fn given_tier_1_referred_users_then_tier_level_1() {
        let referral_code = "DUMMY".to_string();
        let referral_tier = calculate_referral_tier_inner(
            create_dummy_referrals(10, dec!(1001)),
            create_dummy_tiers(),
            referral_code.clone(),
        )
        .unwrap();

        assert_eq!(referral_tier.referral_tier, 1);
        assert_eq!(referral_tier.referral_fee_bonus, dec!(0.2));
        assert_eq!(referral_tier.referral_code, referral_code);
        assert_eq!(referral_tier.number_of_activated_referrals, 10);
        assert_eq!(referral_tier.number_of_total_referrals, 10);
    }

    #[test]
    pub fn given_tier_2_referred_users_then_tier_level_2() {
        let referral_code = "DUMMY".to_string();
        let referral_tier = calculate_referral_tier_inner(
            create_dummy_referrals(20, dec!(2001)),
            create_dummy_tiers(),
            referral_code.clone(),
        )
        .unwrap();

        assert_eq!(referral_tier.referral_tier, 2);
        assert_eq!(referral_tier.number_of_activated_referrals, 20);
        assert_eq!(referral_tier.number_of_total_referrals, 20);
    }

    #[test]
    pub fn given_tier_1_and_not_enough_tier_2_referred_users_then_tier_level_1() {
        let referral_code = "DUMMY".to_string();
        let mut tier_1 = create_dummy_referrals(10, dec!(1001));
        let mut tier_2 = create_dummy_referrals(10, dec!(2001));
        tier_1.append(&mut tier_2);
        let referral_tier =
            calculate_referral_tier_inner(tier_1, create_dummy_tiers(), referral_code).unwrap();

        assert_eq!(referral_tier.referral_tier, 1);
        assert_eq!(referral_tier.number_of_activated_referrals, 10);
        assert_eq!(referral_tier.number_of_total_referrals, 20);
    }

    #[test]
    pub fn given_tier_1_and_not_enough_tier_3_referred_users_then_tier_level_1() {
        let referral_code = "DUMMY".to_string();
        let mut tier_1 = create_dummy_referrals(10, dec!(1001));
        let mut tier_2 = create_dummy_referrals(10, dec!(3001));
        tier_1.append(&mut tier_2);
        let referral_tier =
            calculate_referral_tier_inner(tier_1, create_dummy_tiers(), referral_code).unwrap();

        assert_eq!(referral_tier.referral_tier, 1);
        assert_eq!(referral_tier.number_of_activated_referrals, 10);
        assert_eq!(referral_tier.number_of_total_referrals, 20);
    }

    #[test]
    pub fn given_not_enough_tier_1_and_but_enough_tier_3_referred_users_then_tier_level_3() {
        let referral_code = "DUMMY".to_string();
        let mut tier_1 = create_dummy_referrals(5, dec!(1001));
        let mut tier_2 = create_dummy_referrals(40, dec!(3001));
        tier_1.append(&mut tier_2);
        let referral_tier =
            calculate_referral_tier_inner(tier_1, create_dummy_tiers(), referral_code).unwrap();

        assert_eq!(referral_tier.referral_tier, 3);
        assert_eq!(referral_tier.number_of_activated_referrals, 40);
        assert_eq!(referral_tier.number_of_total_referrals, 45);
    }

    fn create_dummy_referrals(
        number_of_users: usize,
        volume_per_user: Decimal,
    ) -> Vec<UserReferralSummaryView> {
        let mut vec = vec![];
        for _ in 0..number_of_users {
            vec.push(UserReferralSummaryView {
                referring_user: "dummy".to_string(),
                referring_user_referral_code: "dummy".to_string(),
                referred_user: "dummy".to_string(),
                referred_user_referral_code: "dummy".to_string(),
                timestamp: OffsetDateTime::now_utc(),
                referred_user_total_quantity: volume_per_user.to_f32().expect("to fit into f32"),
            })
        }

        vec
    }

    fn create_dummy_tiers() -> Vec<ReferralTier> {
        vec![
            ReferralTier {
                id: 0,
                tier_level: 0,
                min_users_to_refer: 0,
                min_volume_per_referral: 0,
                fee_rebate: 0.0,
                number_of_trades: 10,
                active: true,
            },
            ReferralTier {
                id: 1,
                tier_level: 1,
                min_users_to_refer: 10,
                min_volume_per_referral: 1000,
                fee_rebate: 0.2,
                number_of_trades: 10,
                active: true,
            },
            ReferralTier {
                id: 2,
                tier_level: 2,
                min_users_to_refer: 20,
                min_volume_per_referral: 2000,
                fee_rebate: 0.3,
                number_of_trades: 10,
                active: true,
            },
            ReferralTier {
                id: 3,
                tier_level: 3,
                min_users_to_refer: 30,
                min_volume_per_referral: 3000,
                fee_rebate: 0.3,
                number_of_trades: 10,
                active: true,
            },
        ]
    }
}
