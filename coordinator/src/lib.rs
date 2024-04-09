use anyhow::anyhow;
use anyhow::Result;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;
use dlc_manager::DlcChannelId;
use hex::FromHex;
use lightning::ln::ChannelId;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;

mod collaborative_revert;
mod payout_curve;

pub mod admin;
pub mod backup;
pub mod campaign;
pub mod check_version;
pub mod cli;
pub mod db;
pub mod dlc_handler;
pub mod dlc_protocol;
mod emergency_kit;
mod leaderboard;
pub mod logger;
pub mod message;
pub mod metrics;
pub mod node;
pub mod notifications;
pub mod orderbook;
pub mod position;
pub mod referrals;
pub mod routes;
pub mod routing_fee;
pub mod scheduler;
pub mod schema;
pub mod settings;
pub mod storage;
pub mod trade;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_migration(conn: &mut PgConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrations to succeed");
}

/// Our app's top level error type.
#[derive(Debug)]
pub enum AppError {
    InternalServerError(String),
    BadRequest(String),
    ServiceUnavailable(String),
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "".to_string()),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

pub fn parse_channel_id(channel_id: &str) -> Result<ChannelId> {
    let channel_id = hex::decode(channel_id)?
        .try_into()
        .map_err(|_| anyhow!("Could not parse channel ID"))?;

    Ok(ChannelId(channel_id))
}

pub fn parse_dlc_channel_id(channel_id: &str) -> Result<DlcChannelId> {
    Ok(DlcChannelId::from_hex(channel_id)?)
}

pub fn compute_relative_contracts(contracts: Decimal, direction: &::trade::Direction) -> Decimal {
    match direction {
        ::trade::Direction::Long => contracts,
        ::trade::Direction::Short => -contracts,
    }
}

#[track_caller]
pub fn decimal_from_f32(float: f32) -> Decimal {
    Decimal::from_f32(float).expect("f32 to fit into Decimal")
}

#[track_caller]
pub fn f32_from_decimal(decimal: Decimal) -> f32 {
    decimal.to_f32().expect("Decimal to fit into f32")
}
