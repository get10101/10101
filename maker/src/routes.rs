use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::Json;
use axum::Router;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::Wallet;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;

pub struct AppState {
    pub node: Arc<Node>,
    pub pool: r2d2::Pool<ConnectionManager<PgConnection>>,
}

pub fn router(node: Arc<Node>, pool: Pool<ConnectionManager<PgConnection>>) -> Router {
    let app_state = Arc::new(AppState { node, pool });

    Router::new()
        .route("/", get(index))
        .route("/api/newaddress", get(get_new_address))
        .route("/api/balance", get(get_balance))
        .route("/api/invoice", get(get_invoice))
        .route("/api/channels", get(list_channels).post(create_channel))
        .with_state(app_state)
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
    let address =
        app_state.node.wallet.get_new_address().map_err(|e| {
            AppError::InternalServerError(format!("Failed to get new address: {e:#}"))
        })?;

    let offchain = app_state.node.get_ldk_balance();
    let onchain = app_state
        .node
        .get_on_chain_balance()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?;

    let amount = 2000;
    let invoice = app_state
        .node
        .create_invoice(amount)
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

pub async fn get_new_address(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<String>, AppError> {
    let address =
        app_state.node.wallet.get_new_address().map_err(|e| {
            AppError::InternalServerError(format!("Failed to get new address: {e:#}"))
        })?;
    Ok(Json(address.to_string()))
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
        .create_invoice(2000)
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

    state.node.connect_to_peer(peer).await.map_err(|e| {
        AppError::InternalServerError(format!("Could not connect to target node {e:#}"))
    })?;

    let channel_id = state
        .node
        .initiate_open_channel(peer, channel_amount, initial_send_amount)
        .map_err(|e| AppError::InternalServerError(format!("Failed to open channel: {e:#}")))?;

    Ok(Json(hex::encode(channel_id)))
}

#[derive(Serialize, Debug)]
pub struct ChannelDetail {
    pub channel_id: String,
    pub counterparty: String,
    pub funding_txo: Option<String>,
    pub channel_type: Option<String>,
    pub channel_value_satoshis: u64,
    pub unspendable_punishment_reserve: Option<u64>,
    pub user_channel_id: u128,
    pub balance_msat: u64,
    pub outbound_capacity_msat: u64,
    pub next_outbound_htlc_limit_msat: u64,
    pub inbound_capacity_msat: u64,
    pub confirmations_required: Option<u32>,
    pub force_close_spend_delay: Option<u16>,
    pub is_outbound: bool,
    pub is_channel_ready: bool,
    pub is_usable: bool,
    pub is_public: bool,
    pub inbound_htlc_minimum_msat: Option<u64>,
    pub inbound_htlc_maximum_msat: Option<u64>,
    pub config: Option<ChannelConfig>,
}

#[derive(Serialize, Debug)]
pub struct ChannelConfig {
    pub forwarding_fee_proportional_millionths: u32,
    pub forwarding_fee_base_msat: u32,
    pub cltv_expiry_delta: u16,
    pub max_dust_htlc_exposure_msat: u64,
    pub force_close_avoidance_max_fee_satoshis: u64,
}

pub async fn list_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChannelDetail>>, AppError> {
    let usable_channels = state
        .node
        .list_channels()
        .iter()
        .map(|cd| ChannelDetail {
            channel_id: hex::encode(cd.channel_id),
            counterparty: cd.counterparty.node_id.to_hex(),
            funding_txo: cd
                .funding_txo
                .map(|ft| format!("{}:{}", ft.txid.to_hex(), ft.index)),
            channel_type: cd.channel_type.clone().map(|ct| ct.to_string()),
            channel_value_satoshis: cd.channel_value_satoshis,
            unspendable_punishment_reserve: cd.unspendable_punishment_reserve,
            user_channel_id: cd.user_channel_id,
            balance_msat: cd.balance_msat,
            outbound_capacity_msat: cd.outbound_capacity_msat,
            next_outbound_htlc_limit_msat: cd.next_outbound_htlc_limit_msat,
            inbound_capacity_msat: cd.inbound_capacity_msat,
            confirmations_required: cd.confirmations_required,
            force_close_spend_delay: cd.force_close_spend_delay,
            is_outbound: cd.is_outbound,
            is_channel_ready: cd.is_channel_ready,
            is_usable: cd.is_usable,
            is_public: cd.is_public,
            inbound_htlc_minimum_msat: cd.inbound_htlc_minimum_msat,
            inbound_htlc_maximum_msat: cd.inbound_htlc_maximum_msat,
            config: cd.config.map(|c| ChannelConfig {
                forwarding_fee_proportional_millionths: c.forwarding_fee_proportional_millionths,
                forwarding_fee_base_msat: c.forwarding_fee_base_msat,
                cltv_expiry_delta: c.cltv_expiry_delta,
                max_dust_htlc_exposure_msat: c.max_dust_htlc_exposure_msat,
                force_close_avoidance_max_fee_satoshis: c.force_close_avoidance_max_fee_satoshis,
            }),
        })
        .collect::<Vec<_>>();

    Ok(Json(usable_channels))
}
