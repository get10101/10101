// These modules need to be define at the top so that FRB doesn't try to import from them.
pub mod db;
pub mod ln_dlc;
pub mod trade;

pub mod api;
pub mod calculations;
pub mod commons;
pub mod config;
pub mod event;
pub mod health;
pub mod logger;
pub mod schema;

mod channel_fee;
mod orderbook;

#[allow(
    clippy::all,
    clippy::unwrap_used,
    unused_import_braces,
    unused_qualifications
)]
mod bridge_generated;
mod destination;
