// These modules need to be define at the top so that FRB doesn't try to import from them.
pub mod api;
pub mod calculations;
pub mod channel_trade_constraints;
pub mod commons;
pub mod config;
pub mod db;
pub mod dlc;
pub mod event;
pub mod health;
pub mod logger;
pub mod schema;
pub mod state;
pub mod trade;
pub mod watcher;

mod backup;
mod cipher;
mod destination;
mod dlc_channel;
mod emergency_kit;
mod max_quantity;
mod names;
mod orderbook;
mod polls;
mod report_error;
mod storage;

pub use dlc::get_maintenance_margin_rate;
pub use report_error::report_error_to_coordinator;

#[allow(
    clippy::all,
    clippy::unwrap_used,
    unused_import_braces,
    unused_qualifications
)]
mod bridge_generated;
mod hodl_invoice;
mod position;
