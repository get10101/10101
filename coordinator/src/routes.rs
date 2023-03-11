use crate::node::Node;
use crate::orderbook::routes::delete_order;
use crate::orderbook::routes::fake_match;
use crate::orderbook::routes::get_order;
use crate::orderbook::routes::get_orders;
use crate::orderbook::routes::post_order;
use crate::orderbook::routes::put_order;
use crate::orderbook::routes::websocket_handler;
use crate::orderbook::routes::PriceFeedMessage;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct AppState {
    pub node: Node,
    // Channel used to send messages to all connected clients.
    pub tx_pricefeed: broadcast::Sender<PriceFeedMessage>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
}

pub fn router(node: Node, pool: Pool<ConnectionManager<PgConnection>>) -> Router {
    let (tx, _rx) = broadcast::channel(100);
    let app_state = Arc::new(AppState {
        node,
        pool,
        tx_pricefeed: tx,
    });

    Router::new()
        .route("/", get(index))
        .route("/api/fake_scid/:target_node", post(post_fake_scid))
        .route("/api/newaddress", get(get_new_address))
        .route("/api/balance", get(get_balance))
        .route("/api/invoice", get(get_invoice))
        .route("/api/channels", get(list_channels))
        .route("/api/orderbook/orders", get(get_orders).post(post_order))
        .route(
            "/api/orderbook/orders/:order_id",
            get(get_order).put(put_order).delete(delete_order),
        )
        .route("/api/orderbook/websocket", get(websocket_handler))
        // todo: Remove fake match api once the order book matching logic is in place.
        .route("/api/orderbook/fake_match/:target_node", post(fake_match))
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
) -> Result<Json<u64>, AppError> {
    let target_node = target_node.0;
    let target_node: PublicKey = target_node.parse().map_err(|e| {
        AppError::BadRequest(format!(
            "Provided public key {target_node} was not valid: {e:#}"
        ))
    })?;

    Ok(Json(
        app_state.node.inner.create_intercept_scid(target_node),
    ))
}

pub async fn get_new_address(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<String>, AppError> {
    let address =
        app_state.node.inner.get_new_address().map_err(|e| {
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
    let offchain = state.node.inner.get_ldk_balance();
    let onchain = state
        .node
        .inner
        .get_on_chain_balance()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?;
    Ok(Json(Balance {
        offchain: offchain.available,
        onchain: onchain.confirmed,
    }))
}

pub async fn get_invoice(State(state): State<Arc<AppState>>) -> Result<Json<String>, AppError> {
    let invoice =
        state.node.inner.create_invoice(2000).map_err(|e| {
            AppError::InternalServerError(format!("Failed to create invoice: {e:#}"))
        })?;

    Ok(Json(invoice.to_string()))
}

pub async fn list_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChannelDetail>>, AppError> {
    let usable_channels = state
        .node
        .inner
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
