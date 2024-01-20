use crate::subscribers::AppSubscribers;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use native::api;
use serde::Serialize;
use std::sync::Arc;

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

#[derive(Serialize)]
pub struct Balance {
    on_chain: u64,
    off_chain: u64,
}

pub async fn get_balance(State(subscribers): State<Arc<AppSubscribers>>) -> impl IntoResponse {
    let balance = subscribers
        .wallet_info()
        .map(|wallet_info| Balance {
            on_chain: wallet_info.balances.on_chain,
            off_chain: wallet_info.balances.off_chain,
        })
        .unwrap_or(Balance {
            on_chain: 0,
            off_chain: 0,
        });
    
    Json(balance)
}
