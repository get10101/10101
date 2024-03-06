use bitcoin::secp256k1::PublicKey;
use rust_decimal::prelude::ToPrimitive;
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
pub use order_matching_fee::order_matching_fee_taker;
pub use order_matching_fee::taker_fee;
pub use polls::*;
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
}
