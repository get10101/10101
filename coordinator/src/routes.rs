use axum::extract::Path;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::Wallet;
use http_api_problem::HttpApiProblem;
use http_api_problem::StatusCode;
use ln_dlc_node::node::Node;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

pub struct AppState {
    pub node: Arc<Node>,
}

pub fn router(node: Arc<Node>) -> Router {
    let app_state = Arc::new(AppState { node });

    Router::new()
        .route("/", get(index))
        .route("/api/fake_scid/:target_node", post(post_fake_scid))
        .route("/api/newaddress", get(get_new_address))
        .route("/api/balance", get(get_balance))
        .route("/api/invoice", get(get_invoice))
        .with_state(app_state)
}

#[derive(serde::Serialize)]
struct HelloWorld {
    hello: String,
}

pub async fn index() -> impl IntoResponse {
    Json(HelloWorld {
        hello: "world".to_string(),
    })
}

pub async fn post_fake_scid(
    target_node: Path<String>,
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let target_node = target_node.0;
    let target_node: PublicKey = target_node
        .parse()
        .map_err(|e| {
            HttpApiProblem::new(StatusCode::BAD_REQUEST)
                .title("Invalid public key")
                .detail(format!(
                    "Provided public key {target_node} was not valid: {e:#}"
                ))
        })
        .unwrap();

    Json(app_state.node.create_intercept_scid(target_node))
}

pub async fn get_new_address(State(app_state): State<Arc<AppState>>) -> impl IntoResponse {
    let address = app_state
        .node
        .wallet
        .get_new_address()
        .map_err(|e| {
            HttpApiProblem::new(StatusCode::INTERNAL_SERVER_ERROR)
                .title("Invalid public key")
                .detail(format!("Failed to get new address: {e:#}"))
        })
        .unwrap();
    Json(address.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    offchain: u64,
    onchain: u64,
}

pub async fn get_balance(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let offchain = state.node.get_ldk_balance();
    let onchain = state
        .node
        .get_on_chain_balance()
        .map_err(|e| {
            HttpApiProblem::new(StatusCode::INTERNAL_SERVER_ERROR)
                .title("Invalid public key")
                .detail(format!("Failed to get balance: {e:#}"))
        })
        .unwrap();
    Json(Balance {
        offchain: offchain.available,
        onchain: onchain.confirmed,
    })
}

pub async fn get_invoice(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let invoice = state
        .node
        .create_invoice(2000)
        .map_err(|e| {
            HttpApiProblem::new(StatusCode::INTERNAL_SERVER_ERROR)
                .detail(format!("Failed to create invoice: {e:#}"))
        })
        .unwrap();

    Json(invoice.to_string())
}
