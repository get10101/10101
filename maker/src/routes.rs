use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::ChannelDetails;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;

pub struct AppState {
    pub node: Arc<Node<InMemoryStore>>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
}

pub fn router(
    node: Arc<Node<InMemoryStore>>,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Router {
    let app_state = Arc::new(AppState { node, pool });

    Router::new()
        .route("/", get(index))
        .route("/api/newaddress", get(get_unused_address))
        .route("/api/balance", get(get_balance))
        .route("/api/invoice", get(get_invoice))
        .route("/api/channels", get(list_channels).post(create_channel))
        .route("/api/connect", post(connect_to_peer))
        .route("/api/pay-invoice/:invoice", post(pay_invoice))
        .with_state(app_state)
}

pub async fn connect_to_peer(
    State(state): State<Arc<AppState>>,
    target: Json<NodeInfo>,
) -> Result<(), AppError> {
    let target = target.0;
    state.node.connect(target).await.map_err(|err| {
        AppError::InternalServerError(format!("Could not connect to {target}. Error: {err}"))
    })?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct Index {
    address: String,
    balance: Balance,
    invoice: Invoice,
}

#[derive(serde::Serialize)]
pub struct Invoice {
    invoice: String,
    amount: u64,
}

pub async fn index(State(app_state): State<Arc<AppState>>) -> Result<Json<Index>, AppError> {
    let address = app_state.node.get_unused_address();

    let offchain = app_state.node.get_ldk_balance();
    let onchain = app_state
        .node
        .get_on_chain_balance()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?;

    let amount = 2000;
    let invoice = app_state
        .node
        .create_invoice(amount, "".to_string(), 180)
        .map_err(|e| AppError::InternalServerError(format!("Failed to create invoice: {e:#}")))?;

    Ok(Json(Index {
        address: address.to_string(),
        balance: Balance {
            offchain: offchain.available,
            onchain: onchain.confirmed,
        },
        invoice: Invoice {
            invoice: invoice.to_string(),
            amount,
        },
    }))
}

pub async fn get_unused_address(State(app_state): State<Arc<AppState>>) -> Json<String> {
    Json(app_state.node.get_unused_address().to_string())
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    offchain: u64,
    onchain: u64,
}

pub async fn get_balance(State(state): State<Arc<AppState>>) -> Result<Json<Balance>, AppError> {
    let offchain = state.node.get_ldk_balance();
    let onchain = state
        .node
        .get_on_chain_balance()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?;
    Ok(Json(Balance {
        offchain: offchain.available,
        onchain: onchain.confirmed,
    }))
}

pub async fn get_invoice(State(state): State<Arc<AppState>>) -> Result<Json<String>, AppError> {
    let invoice = state
        .node
        .create_invoice(2000, "".to_string(), 180)
        .map_err(|e| AppError::InternalServerError(format!("Failed to create invoice: {e:#}")))?;

    Ok(Json(invoice.to_string()))
}

/// Our app's top level error type.
pub enum AppError {
    InternalServerError(String),
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

#[derive(Deserialize)]
pub struct ChannelParams {
    target: TargetInfo,
    local_balance: u64,
    remote_balance: Option<u64>,
}

#[derive(Deserialize)]
pub struct TargetInfo {
    pubkey: String,
    address: String,
}

pub async fn create_channel(
    State(state): State<Arc<AppState>>,
    channel_params: Json<ChannelParams>,
) -> Result<Json<String>, AppError> {
    let target_address =
        channel_params.0.target.address.parse().map_err(|e| {
            AppError::BadRequest(format!("Invalid target node address provided {e:#}"))
        })?;
    let peer = NodeInfo {
        pubkey: PublicKey::from_str(channel_params.0.target.pubkey.as_str()).map_err(|e| {
            AppError::BadRequest(format!("Invalid target node pubkey provided {e:#}"))
        })?,
        address: target_address,
    };

    let channel_amount = channel_params.local_balance;
    let initial_send_amount = channel_params.remote_balance.unwrap_or_default();

    state.node.connect(peer).await.map_err(|e| {
        AppError::InternalServerError(format!("Could not connect to target node {e:#}"))
    })?;

    let channel_id = state
        .node
        .initiate_open_channel(peer.pubkey, channel_amount, initial_send_amount, false)
        .map_err(|e| AppError::InternalServerError(format!("Failed to open channel: {e:#}")))?;

    Ok(Json(hex::encode(channel_id)))
}

pub async fn list_channels(State(state): State<Arc<AppState>>) -> Json<Vec<ChannelDetails>> {
    let channels = state
        .node
        .list_channels()
        .into_iter()
        .map(ChannelDetails::from)
        .collect::<Vec<_>>();

    Json(channels)
}

pub async fn pay_invoice(
    State(state): State<Arc<AppState>>,
    Path(invoice): Path<String>,
) -> Result<Json<String>, AppError> {
    let invoice = invoice
        .parse()
        .map_err(|e| AppError::BadRequest(format!("Invalid invoice provided {e:#}")))?;
    state
        .node
        .send_payment(&invoice)
        .map_err(|e| AppError::InternalServerError(format!("Could not pay invoice {e:#}")))?;
    Ok(Json("bl".to_string()))
}
