use crate::health::Health;
use crate::health::OverallMakerHealth;
use crate::position;
use crate::position::ContractSymbol;
use crate::position::GetPosition;
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
use lightning::ln::msgs::NetAddress;
use ln_dlc_node::node::peer_manager::alias_as_bytes;
use ln_dlc_node::node::peer_manager::broadcast_node_announcement;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::ChannelDetails;
use opentelemetry_prometheus::PrometheusExporter;
use prometheus::Encoder;
use prometheus::TextEncoder;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::spawn_blocking;

pub struct AppState {
    node: Arc<Node<InMemoryStore>>,
    exporter: PrometheusExporter,
    position_manager: xtra::Address<position::Manager>,
    announcement_addresses: Vec<NetAddress>,
    node_alias: String,
    health: Health,
}

pub fn router(
    node: Arc<Node<InMemoryStore>>,
    exporter: PrometheusExporter,
    position_manager: xtra::Address<position::Manager>,
    health: Health,
    announcement_addresses: Vec<NetAddress>,
    node_alias: &str,
) -> Router {
    let app_state = Arc::new(AppState {
        node,
        exporter,
        position_manager,
        health,
        announcement_addresses,
        node_alias: node_alias.to_string(),
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
        .route("/api/position", get(get_position))
        .route("/api/node", get(get_node_info))
        .route("/metrics", get(get_metrics))
        .route("/health", get(get_health))
        .route(
            "/api/broadcast_announcement",
            post(post_broadcast_announcement),
        )
        .with_state(app_state)
}

pub async fn get_node_info(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<NodeInfo>, AppError> {
    let node_info = app_state.node.info;
    Ok(Json(node_info))
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

#[derive(Serialize)]
pub struct Position {
    tentenone: HashSet<PositionForContractSymbol>,
}

#[derive(Serialize, Eq)]
pub struct PositionForContractSymbol {
    contract_symbol: ContractSymbol,
    contracts: Decimal,
}

impl Hash for PositionForContractSymbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.contract_symbol.hash(state);
    }
}

impl PartialEq for PositionForContractSymbol {
    fn eq(&self, other: &PositionForContractSymbol) -> bool {
        self.contract_symbol == other.contract_symbol
    }
}

pub async fn get_position(State(state): State<Arc<AppState>>) -> Result<Json<Position>, AppError> {
    let position = state
        .position_manager
        .send(GetPosition)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to get position: {e:#}")))?;

    let position = HashSet::from_iter(position.tentenone.into_iter().map(
        |(contract_symbol, contracts)| PositionForContractSymbol {
            contract_symbol,
            contracts,
        },
    ));

    Ok(Json(Position {
        tentenone: position,
    }))
}

pub async fn get_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let autometrics = match autometrics::prometheus_exporter::encode_to_string() {
        Ok(metrics) => metrics,
        Err(err) => {
            tracing::error!("Could not collect autometrics {err:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err));
        }
    };

    let exporter = state.exporter.clone();
    let encoder = TextEncoder::new();
    let metric_families = exporter.registry().gather();
    let mut result = vec![];
    match encoder.encode(&metric_families, &mut result) {
        Ok(()) => (),
        Err(err) => {
            tracing::error!("Could not collect opentelemetry metrics {err:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err));
        }
    };

    let open_telemetry_metrics = match String::from_utf8(result) {
        Ok(s) => s,
        Err(err) => {
            tracing::error!("Could not format metrics as string {err:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err));
        }
    };

    (StatusCode::OK, open_telemetry_metrics + &autometrics)
}

/// Returns 500 if any of the vital services are offline
pub async fn get_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OverallMakerHealth>, AppError> {
    let resp = state
        .health
        .get_health()
        .map_err(|e| AppError::InternalServerError(format!("Error: {e:#}")))?;
    Ok(Json(resp))
}

pub async fn post_broadcast_announcement(
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let node_alias = alias_as_bytes(state.node_alias.as_str()).map_err(|e| {
        AppError::InternalServerError(format!(
            "Could not parse node alias {0} due to {e:#}",
            state.node_alias
        ))
    })?;
    broadcast_node_announcement(
        &state.node.peer_manager,
        node_alias,
        state.announcement_addresses.clone(),
    );

    Ok(())
}
