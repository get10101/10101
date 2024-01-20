use axum::response::IntoResponse;
use axum::Json;
use native::api;
use serde::Serialize;

#[derive(Serialize)]
pub struct Version {
    version: String,
}

pub async fn version() -> Json<Version> {
    Json(Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn get_unused_address() -> impl IntoResponse {
    api::get_unused_address().0
}
