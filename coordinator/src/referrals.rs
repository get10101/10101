use crate::db;
use crate::db::bonus_status::BonusStatus;
use crate::db::bonus_status::BonusType;
use crate::db::bonus_tiers::BonusTier;
use crate::db::bonus_tiers::UserReferralSummaryView;
use crate::db::user::User;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::BonusStatusType;
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
    let bonus_status = db::bonus_status::active_status_for_user(connection, &trader_pubkey)?;
    let user = db::user::get_user(connection, &trader_pubkey)?.context("User not found")?;

    // first we check for promo, promo always has highest priority
    if let Some(status) = has_active_promo_bonus(connection, &bonus_status, &user)? {
        return Ok(status);
    }

    // then we check if he was referred, if he has been, his referral bonus might still be active
    // first we check for promo, promo always has highest priority
    if let Some(status) = has_active_referent_bonus(connection, &bonus_status, &user)? {
        return Ok(status);
    }

    // if none of the above, we check for the highest tier without promo or referent
    let mut filtered_boni = bonus_status
        .iter()
        .filter(|bonus| {
            bonus.bonus_type != BonusType::Promotion && bonus.bonus_type != BonusType::Referent
        })
        .collect::<Vec<_>>();

    // we sort by tier_level, higher tier means better bonus
    filtered_boni.sort_by(|a, b| b.tier_level.cmp(&a.tier_level));

    // next we pick the highest
    if let Some(bonus) = filtered_boni.first() {
        let referrals =
            db::bonus_tiers::all_referrals_by_referring_user(connection, &trader_pubkey)?;
        let tier = db::bonus_tiers::tier_by_tier_level(connection, bonus.tier_level)?;

        return Ok(ReferralStatus {
            referral_code: user.referral_code,
            number_of_activated_referrals: referrals
                .iter()
                .filter(|referral| {
                    referral.referred_user_total_quantity.floor() as i32
                        > tier.min_volume_per_referral
                })
                .count(),
            number_of_total_referrals: referrals.len(),
            referral_tier: tier.tier_level as usize,
            referral_fee_bonus: Decimal::from_f32(tier.fee_rebate).expect("to fit"),
            bonus_status_type: Some(bonus.bonus_type.into()),
        });
    }

    // None of the above, user is a boring normal user
    Ok(ReferralStatus::new(trader_pubkey))
}

fn has_active_referent_bonus(
    connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
    bonus_status: &Vec<BonusStatus>,
    user: &User,
) -> Result<Option<ReferralStatus>> {
    if let Some(bonus) = bonus_status
        .iter()
        .find(|status| BonusType::Referent == status.bonus_type)
    {
        tracing::debug!(
            trader_pubkey = bonus.trader_pubkey,
            bonus_type = ?bonus.bonus_type,
            tier_level = bonus.tier_level,
            "Trader was referred");
        let tier = db::bonus_tiers::tier_by_tier_level(connection, bonus.tier_level)?;

        return Ok(Some(ReferralStatus {
            referral_code: user.referral_code.clone(),
            number_of_activated_referrals: 0,
            number_of_total_referrals: 0,
            referral_tier: tier.tier_level as usize,
            referral_fee_bonus: Decimal::from_f32(tier.fee_rebate).expect("to fit"),
            bonus_status_type: Some(bonus.bonus_type.into()),
        }));
    }
    Ok(None)
}

fn has_active_promo_bonus(
    connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
    bonus_status: &Vec<BonusStatus>,
    user: &User,
) -> Result<Option<ReferralStatus>> {
    if let Some(bonus) = bonus_status
        .iter()
        .find(|status| BonusType::Promotion == status.bonus_type)
    {
        tracing::debug!(
            trader_pubkey = bonus.trader_pubkey,
            bonus_type = ?bonus.bonus_type,
            tier_level = bonus.tier_level,
            "Trader was part of a promo");
        let tier = db::bonus_tiers::tier_by_tier_level(connection, bonus.tier_level)?;

        return Ok(Some(ReferralStatus {
            referral_code: user.referral_code.clone(),
            number_of_activated_referrals: 0,
            number_of_total_referrals: 0,
            referral_tier: tier.tier_level as usize,
            referral_fee_bonus: Decimal::from_f32(tier.fee_rebate).expect("to fit"),
            bonus_status_type: Some(bonus.bonus_type.into()),
        }));
    }

    Ok(None)
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

    // first we check his existing status. If he currently has an active referent status or active
    // promo status we return here
    let status = get_referral_status(trader_pubkey, connection)?;
    if let Some(bonus_level) = &status.bonus_status_type {
        if BonusStatusType::Referent == *bonus_level || BonusStatusType::Promotion == *bonus_level {
            tracing::debug!(
                trader_pubkey = trader_pubkey_str,
                bonus_tier = status.referral_tier,
                bonus_level = ?bonus_level,
                "User has active bonus status"
            );
            return Ok(status);
        }
    }

    let user = db::user::get_user(connection, &trader_pubkey)?.context("User not found")?;
    let referrals = db::bonus_tiers::all_referrals_by_referring_user(connection, &trader_pubkey)?;
    let bonus_tiers = db::bonus_tiers::all_active_by_type(
        connection,
        vec![
            BonusType::Referral,
            BonusType::Promotion,
            BonusType::Referent,
        ],
    )?;

    let total_referrals = referrals.len();

    let referral_code = user.referral_code;
    if total_referrals > 0 {
        let referral_tier = calculate_bonus_status_inner(referrals.clone(), bonus_tiers.clone())?;
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
                .filter(|referral| {
                    referral.referred_user_total_quantity.floor() as i32
                        > maybe_bonus_tier.min_volume_per_referral
                })
                .count(),
            number_of_total_referrals: total_referrals,
            referral_tier: maybe_bonus_tier.tier_level as usize,
            referral_fee_bonus: Decimal::from_f32(maybe_bonus_tier.fee_rebate).expect("to fit"),
            bonus_status_type: Some(maybe_bonus_tier.bonus_tier_type.into()),
        });
    }

    tracing::debug!(
        trader_pubkey = trader_pubkey.to_string(),
        "Trader doesn't have any referral status yet"
    );

    // User doesn't have any referral status yet
    Ok(ReferralStatus {
        referral_code,
        number_of_activated_referrals: 0,
        number_of_total_referrals: 0,
        referral_tier: 0,
        referral_fee_bonus: Decimal::ZERO,
        bonus_status_type: None,
    })
}

/// Returns the tier_level of the calculated tier
fn calculate_bonus_status_inner(
    referrals: Vec<UserReferralSummaryView>,
    bonus_tiers: Vec<BonusTier>,
) -> Result<i32> {
    let mut referred_users_sorted_by_tier: HashMap<i32, i32> = HashMap::new();

    let mut bonus_tiers = bonus_tiers;

    // we sort descending by volume so that we can pick the highest suitable tier below
    bonus_tiers.sort_by(|a, b| b.min_volume_per_referral.cmp(&a.min_volume_per_referral));

    for referred_user in referrals {
        let volume = referred_user.referred_user_total_quantity;
        if let Some(tier) = bonus_tiers
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
    for tier in bonus_tiers {
        if let Some(number_of_users) = referred_users_sorted_by_tier.get(&tier.tier_level) {
            if *number_of_users >= tier.min_users_to_refer {
                selected_tier.replace(tier);
                break;
            }
        }
    }

    Ok(selected_tier
        .clone()
        .map(|t| t.tier_level)
        .unwrap_or_default())
}

#[cfg(test)]
pub mod tests {
    use crate::db::bonus_status::BonusType;
    use crate::db::bonus_tiers::BonusTier;
    use crate::db::bonus_tiers::UserReferralSummaryView;
    use crate::referrals::calculate_bonus_status_inner;
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::OffsetDateTime;

    #[test]
    pub fn given_no_referred_users_then_tier_level_0() {
        let referral_tier = calculate_bonus_status_inner(vec![], create_dummy_tiers()).unwrap();

        assert_eq!(referral_tier, 0);
    }

    #[test]
    pub fn given_tier_1_referred_users_then_tier_level_1() {
        let referral_tier = calculate_bonus_status_inner(
            create_dummy_referrals(10, dec!(1001)),
            create_dummy_tiers(),
        )
        .unwrap();

        assert_eq!(referral_tier, 1);
    }

    #[test]
    pub fn given_tier_2_referred_users_then_tier_level_2() {
        let referral_tier = calculate_bonus_status_inner(
            create_dummy_referrals(20, dec!(2001)),
            create_dummy_tiers(),
        )
        .unwrap();

        assert_eq!(referral_tier, 2);
    }

    #[test]
    pub fn given_tier_1_and_not_enough_tier_2_referred_users_then_tier_level_1() {
        let mut tier_1 = create_dummy_referrals(10, dec!(1001));
        let mut tier_2 = create_dummy_referrals(10, dec!(2001));
        tier_1.append(&mut tier_2);
        let referral_tier = calculate_bonus_status_inner(tier_1, create_dummy_tiers()).unwrap();

        assert_eq!(referral_tier, 1);
    }

    #[test]
    pub fn given_tier_1_and_not_enough_tier_3_referred_users_then_tier_level_1() {
        let mut tier_1 = create_dummy_referrals(10, dec!(1001));
        let mut tier_2 = create_dummy_referrals(10, dec!(3001));
        tier_1.append(&mut tier_2);
        let referral_tier = calculate_bonus_status_inner(tier_1, create_dummy_tiers()).unwrap();

        assert_eq!(referral_tier, 1);
    }

    #[test]
    pub fn given_not_enough_tier_1_and_but_enough_tier_3_referred_users_then_tier_level_3() {
        let mut tier_1 = create_dummy_referrals(5, dec!(1001));
        let mut tier_2 = create_dummy_referrals(40, dec!(3001));
        tier_1.append(&mut tier_2);
        let referral_tier = calculate_bonus_status_inner(tier_1, create_dummy_tiers()).unwrap();

        assert_eq!(referral_tier, 3);
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

    fn create_dummy_tiers() -> Vec<BonusTier> {
        vec![
            BonusTier {
                id: 0,
                tier_level: 0,
                min_users_to_refer: 0,
                min_volume_per_referral: 0,
                fee_rebate: 0.0,
                number_of_trades: 10,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
            BonusTier {
                id: 1,
                tier_level: 1,
                min_users_to_refer: 10,
                min_volume_per_referral: 1000,
                fee_rebate: 0.2,
                number_of_trades: 10,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
            BonusTier {
                id: 2,
                tier_level: 2,
                min_users_to_refer: 20,
                min_volume_per_referral: 2000,
                fee_rebate: 0.3,
                number_of_trades: 10,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
            BonusTier {
                id: 3,
                tier_level: 3,
                min_users_to_refer: 30,
                min_volume_per_referral: 3000,
                fee_rebate: 0.3,
                number_of_trades: 10,
                bonus_tier_type: BonusType::Referral,
                active: true,
            },
        ]
    }
}
