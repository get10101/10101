// These modules need to be define at the top so that FRB doesn't try to import from them.
pub mod db;
pub mod ln_dlc;
pub mod trade;

pub mod api;
pub mod calculations;
pub mod commons;
pub mod config;
pub mod dlc;
pub mod event;
pub mod health;
pub mod logger;
pub mod schema;
pub mod state;

mod backup;
pub mod channel_trade_constraints;
mod cipher;
mod destination;
mod dlc_channel;
mod emergency_kit;
mod max_quantity;
mod names;
mod orderbook;
mod polls;
mod storage;

pub use ln_dlc::get_maintenance_margin_rate;

#[allow(
    clippy::all,
    clippy::unwrap_used,
    unused_import_braces,
    unused_qualifications
)]
mod bridge_generated;
mod position;
