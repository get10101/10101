use crate::position;
use crate::position::PositionUpdateTenTenOne;
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
use tokio::task::spawn_blocking;
use xtra::Address;

pub struct AppState {
    node: Arc<Node<InMemoryStore>>,
    position_manager: Address<position::Manager>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
}

pub fn router(
    node: Arc<Node<InMemoryStore>>,
    position_manager: Address<position::Manager>,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Router {
    let app_state = Arc::new(AppState {
        node,
        position_manager,
        pool,
    });

    Router::new()
        .route("/", get(index))
        .route("/api/newaddress", get(get_unused_address))
        .route("/api/balance", get(get_balance))
        .route("/api/invoice", get(get_invoice))
        .route("/api/channels", get(list_channels).post(create_channel))
        .route("/api/connect", post(connect_to_peer))
        .route("/api/pay-invoice/:invoice", post(pay_invoice))
        .route("/api/sync-on-chain", post(sync_on_chain))
        .route(
            "/api/update-simulated-position",
            post(update_simulated_position),
        )
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
            offchain: offchain.available(),
            onchain: onchain.confirmed,
        },
        invoice: Invoice {
            invoice: invoice.to_string(),
            amount,
        },
    }))
}

pub async fn get_unused_address(State(app_state): State<Arc<AppState>>) -> impl IntoResponse {
    app_state.node.get_unused_address().to_string()
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    pub offchain: u64,
    pub onchain: u64,
}

pub async fn get_balance(State(state): State<Arc<AppState>>) -> Result<Json<Balance>, AppError> {
    let offchain = state.node.get_ldk_balance();
    let onchain = state
        .node
        .get_on_chain_balance()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?;
    Ok(Json(Balance {
        offchain: offchain.available(),
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

#[derive(Serialize, Deserialize)]
pub struct ChannelParams {
    pub target: TargetInfo,
    pub local_balance: u64,
    pub remote_balance: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct TargetInfo {
    pub pubkey: String,
    pub address: String,
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
        .initiate_open_channel(peer.pubkey, channel_amount, initial_send_amount, true)
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
) -> Result<(), AppError> {
    let invoice = invoice
        .parse()
        .map_err(|e| AppError::BadRequest(format!("Invalid invoice provided {e:#}")))?;
    state
        .node
        .send_payment(&invoice)
        .map_err(|e| AppError::InternalServerError(format!("Could not pay invoice {e:#}")))?;
    Ok(())
}

pub async fn sync_on_chain(State(state): State<Arc<AppState>>) -> Result<(), AppError> {
    spawn_blocking(move || state.node.wallet().sync())
        .await
        .expect("task to complete")
        .map_err(|e| AppError::InternalServerError(format!("Could not sync wallet: {e:#}")))?;

    Ok(())
}

pub async fn update_simulated_position(
    State(state): State<Arc<AppState>>,
    body: Json<UpdatePositionRequest>,
) -> Result<(), AppError> {
    state
        .position_manager
        .send(PositionUpdateTenTenOne::new(
            body.contract_symbol.into(),
            body.contracts,
        ))
        .await
        .map_err(|e| AppError::InternalServerError(format!("Could not sync wallet: {e:#}")))?;

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct UpdatePositionRequest {
    contract_symbol: ContractSymbol,
    /// The number of contracts corresponding to this 10101 position update.
    ///
    /// The sign determines the direction: positive is long; negative is short.
    contracts: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ContractSymbol {
    BtcUsd,
}

impl From<ContractSymbol> for position::ContractSymbol {
    fn from(value: ContractSymbol) -> Self {
        match value {
            ContractSymbol::BtcUsd => Self::BtcUsd,
        }
    }
}
