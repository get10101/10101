use anyhow::bail;
use bitcoin::secp256k1::PublicKey;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

mod backup;
mod collab_revert;
mod liquidity_option;
mod message;
mod order;
mod order_matching_fee;
mod polls;
mod price;
mod reported_error;
mod rollover;
mod signature;
mod trade;

pub use crate::commons::trade::*;
pub use backup::*;
pub use collab_revert::*;
pub use liquidity_option::*;
pub use message::*;
pub use order::*;
pub use order_matching_fee::order_matching_fee;
pub use polls::*;
pub use price::*;
pub use reported_error::ReportedError;
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
    pub os: Option<String>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HodlInvoiceParams {
    pub trader_pubkey: PublicKey,
    pub amt_sats: u64,
    pub r_hash: String,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContractSymbol {
    BtcUsd,
}

impl ContractSymbol {
    pub fn label(self) -> String {
        match self {
            ContractSymbol::BtcUsd => "btcusd".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Direction {
    Long,
    Short,
}

impl Direction {
    pub fn opposite(&self) -> Direction {
        match self {
            Direction::Long => Direction::Short,
            Direction::Short => Direction::Long,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Direction::Long => "Long",
            Direction::Short => "Short",
        };

        s.fmt(f)
    }
}

impl FromStr for ContractSymbol {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "btcusd" => Ok(ContractSymbol::BtcUsd),
            // BitMEX representation
            "xbtusd" => Ok(ContractSymbol::BtcUsd),
            unknown => bail!("Unknown contract symbol {unknown}"),
        }
    }
}

impl fmt::Display for ContractSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            ContractSymbol::BtcUsd => "btcusd",
        };
        symbol.to_string().fmt(f)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::commons::referral_from_pubkey;
    use crate::commons::ContractSymbol;
    use secp256k1::PublicKey;
    use std::str::FromStr;

    #[test]
    fn contract_symbol_from_str() {
        assert_eq!(
            ContractSymbol::from_str("btcusd").unwrap(),
            ContractSymbol::BtcUsd
        );
        assert_eq!(
            ContractSymbol::from_str("BTCUSD").unwrap(),
            ContractSymbol::BtcUsd
        );
        assert_eq!(
            ContractSymbol::from_str("xbtusd").unwrap(),
            ContractSymbol::BtcUsd
        );
        assert!(ContractSymbol::from_str("dogeusd").is_err());
    }

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
