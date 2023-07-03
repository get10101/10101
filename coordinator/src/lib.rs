pub mod admin;
pub mod cli;
pub mod db;
pub mod logger;
pub mod metrics;
pub mod node;
pub mod orderbook;
pub mod position;
pub mod routes;
pub mod schema;
pub mod settings;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;
use serde_json::json;

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
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NoMatchFound(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::InvalidOrder(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
