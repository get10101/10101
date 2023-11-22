use anyhow::Result;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;
use serde_json::json;
use settings::Settings;

pub mod admin;
pub mod backup;
pub mod cli;
mod collaborative_revert;
pub mod db;
pub mod logger;
pub mod message;
pub mod metrics;
pub mod node;
pub mod notifications;
pub mod orderbook;
mod payout_curve;
pub mod position;
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
    NoMatchFound(String),
    InvalidOrder(String),
    ServiceUnavailable(String),
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NoMatchFound(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::InvalidOrder(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "".to_string()),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

/// Check if the liquidity is sufficient to open a JIT channel from the coordinator
pub fn is_liquidity_sufficient(
    settings: &Settings,
    balance: bdk::Balance,
    amount_sats: u64,
) -> bool {
    balance.get_spendable() >= amount_sats + settings.min_liquidity_threshold_sats
}

pub fn parse_channel_id(channel_id: &str) -> Result<[u8; 32]> {
    let channel_id = hex::decode(channel_id)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Could not parse channel ID"))?;

    Ok(channel_id)
}
