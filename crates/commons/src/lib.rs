use rust_decimal::prelude::ToPrimitive;
use secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;

mod backup;
mod collab_revert;
mod liquidity_option;
mod message;
mod order;
mod order_matching_fee;
mod price;
mod rollover;
mod route;
mod signature;
mod trade;

pub use crate::backup::*;
pub use crate::collab_revert::*;
pub use crate::liquidity_option::*;
pub use crate::message::*;
pub use crate::order::*;
pub use crate::order_matching_fee::order_matching_fee_taker;
pub use crate::price::best_current_price;
pub use crate::price::Price;
pub use crate::price::Prices;
pub use crate::rollover::*;
pub use crate::route::*;
pub use crate::signature::*;
pub use crate::trade::*;

pub const AUTH_SIGN_MESSAGE: &[u8; 19] = b"Hello it's me Mario";

/// Registration details for enrolling into the beta program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParams {
    pub pubkey: PublicKey,
    pub email: Option<String>,
    pub nostr: Option<String>,
}
