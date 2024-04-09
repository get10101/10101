use bitcoin::secp256k1::PublicKey;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;

mod backup;
mod collab_revert;
mod liquidity_option;
mod message;
mod order;
mod order_matching_fee;
mod polls;
mod price;
mod rollover;
mod signature;
mod trade;

pub use crate::trade::*;
pub use backup::*;
pub use collab_revert::*;
pub use liquidity_option::*;
pub use message::*;
pub use order::*;
pub use order_matching_fee::order_matching_fee;
pub use polls::*;
pub use price::best_ask_price;
pub use price::best_bid_price;
pub use price::best_current_price;
pub use price::Price;
pub use price::Prices;
pub use rollover::*;
pub use signature::*;

pub const AUTH_SIGN_MESSAGE: &[u8; 19] = b"Hello it's me Mario";

/// Registration details for enrolling into the beta program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParams {
    pub pubkey: PublicKey,
    pub contact: Option<String>,
    pub nickname: Option<String>,
    pub version: Option<String>,
    /// Entered referral code, i.e. this user was revered by using this referral code
    pub referral_code: Option<String>,
}

/// Registration details for enrolling into the beta program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUsernameParams {
    pub pubkey: PublicKey,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub pubkey: PublicKey,
    pub contact: Option<String>,
    pub nickname: Option<String>,
    pub referral_code: String,
}

impl User {
    pub fn new(
        pubkey: PublicKey,
        contact: Option<String>,
        nickname: Option<String>,
        referral_code: String,
    ) -> Self {
        Self {
            pubkey,
            contact,
            nickname,
            referral_code,
        }
    }
}

pub fn referral_from_pubkey(public_key: PublicKey) -> String {
    let referral_code = public_key
        .to_string()
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .to_uppercase();
    referral_code
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReferralStatus {
    /// your personal referral code
    pub referral_code: String,
    /// These are the referrals which have reached the tier's min trading volume
    pub number_of_activated_referrals: usize,
    /// Total number of referred users
    pub number_of_total_referrals: usize,
    /// The more the user refers, the higher the tier. Tier 0 means no referral
    pub referral_tier: usize,
    /// Activated bonus, a percentage to be subtracted from the matching fee.
    #[serde(with = "rust_decimal::serde::float")]
    pub referral_fee_bonus: Decimal,
    /// The type of this referral status
    pub bonus_status_type: Option<BonusStatusType>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
pub enum BonusStatusType {
    /// The bonus is because he referred enough users
    Referral,
    /// The user has been referred and gets a bonus
    Referent,
}

impl ReferralStatus {
    pub fn new(trader_id: PublicKey) -> Self {
        Self {
            referral_code: referral_from_pubkey(trader_id),
            number_of_activated_referrals: 0,
            number_of_total_referrals: 0,
            referral_tier: 0,
            referral_fee_bonus: Default::default(),
            bonus_status_type: None,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::referral_from_pubkey;
    use secp256k1::PublicKey;
    use std::str::FromStr;

    #[test]
    pub fn test_referral_generation() {
        let pk = PublicKey::from_str(
            "0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166",
        )
        .unwrap();

        let referral = referral_from_pubkey(pk);
        assert_eq!(referral, "DDD166".to_string());
    }
}
