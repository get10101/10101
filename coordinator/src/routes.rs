use crate::db::user;
use crate::node::Node;
use crate::orderbook::routes::delete_order;
use crate::orderbook::routes::get_order;
use crate::orderbook::routes::get_orders;
use crate::orderbook::routes::post_order;
use crate::orderbook::routes::put_order;
use crate::orderbook::routes::websocket_handler;
use crate::AppError;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::delete;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::secp256k1::PublicKey;
use coordinator_commons::RegisterParams;
use coordinator_commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use ln_dlc_node::node::NodeInfo;
use orderbook_commons::FakeScidResponse;
use orderbook_commons::OrderbookMsg;

use crate::admin::close_channel;
use crate::admin::connect_to_peer;
use crate::admin::delete_subchannel;
use crate::admin::get_balance;
use crate::admin::is_connected;
use crate::admin::list_channels;
use crate::admin::list_dlc_channels;
use crate::admin::list_on_chain_transactions;
use crate::admin::list_peers;
use crate::admin::open_channel;
use crate::admin::sign_message;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub struct AppState {
    pub node: Node,
    // Channel used to send messages to all connected clients.
    pub tx_pricefeed: broadcast::Sender<OrderbookMsg>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub authenticated_users: Arc<Mutex<HashMap<PublicKey, mpsc::Sender<OrderbookMsg>>>>,
}

pub fn router(node: Node, pool: Pool<ConnectionManager<PgConnection>>) -> Router {
    let (tx, _rx) = broadcast::channel(100);
    let app_state = Arc::new(AppState {
        node,
        pool,
        tx_pricefeed: tx,
        authenticated_users: Default::default(),
    });

    Router::new()
        .route("/", get(index))
        .route("/api/fake_scid/:target_node", post(post_fake_scid))
        .route(
            "/api/register_invoice/:target_node",
            post(register_interceptable_invoice),
        )
        .route("/api/newaddress", get(get_new_address))
        .route("/api/node", get(get_node_info))
        .route("/api/invoice", get(get_invoice))
        .route("/api/orderbook/orders", get(get_orders).post(post_order))
        .route(
            "/api/orderbook/orders/:order_id",
            get(get_order).put(put_order).delete(delete_order),
        )
        .route("/api/orderbook/websocket", get(websocket_handler))
        .route("/api/trade", post(post_trade))
        .route("/api/register", post(post_register))
        .route("/api/admin/balance", get(get_balance))
        .route("/api/admin/channels", get(list_channels).post(open_channel))
        .route("/api/admin/channels/:channel_id", delete(close_channel))
        .route("/api/admin/peers", get(list_peers))
        .route("/api/admin/dlc_channels", get(list_dlc_channels))
        .route("/api/admin/transactions", get(list_on_chain_transactions))
        .route(
            "/api/admin/dlc_channels/:channel_id",
            delete(delete_subchannel),
        )
        .route("/api/admin/sign/:msg", get(sign_message))
        .route("/api/admin/connect", post(connect_to_peer))
        .route("/api/admin/is_connected/:target_pubkey", get(is_connected))
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
        app_state.node.inner.create_intercept_scid(target_node).scid,
    ))
}

pub async fn register_interceptable_invoice(
    target_node: Path<String>,
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<FakeScidResponse>, AppError> {
    let target_node = target_node.0;
    let target_node: PublicKey = target_node.parse().map_err(|e| {
        AppError::BadRequest(format!(
            "Provided public key {target_node} was not valid: {e:#}"
        ))
    })?;

    let details = app_state.node.inner.create_intercept_scid(target_node);
    let scid = details.scid;
    let fee_rate_millionth = details.jit_routing_fee_millionth;
    Ok(Json(FakeScidResponse {
        scid,
        fee_rate_millionth,
    }))
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

pub async fn get_node_info(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<NodeInfo>, AppError> {
    let node_info = app_state.node.inner.info;
    Ok(Json(node_info))
}

#[derive(Debug, Deserialize)]
pub struct InvoiceParams {
    amount: Option<u64>,
    description: Option<String>,
    expiry: Option<u32>,
}

pub async fn get_invoice(
    Query(params): Query<InvoiceParams>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<String>, AppError> {
    let invoice = state
        .node
        .inner
        .create_invoice(
            params.amount.unwrap_or_default(),
            params.description.unwrap_or_default(),
            params.expiry.unwrap_or(180),
        )
        .map_err(|e| AppError::InternalServerError(format!("Failed to create invoice: {e:#}")))?;

    Ok(Json(invoice.to_string()))
}

// TODO: We might want to have our own ContractInput type here so we can potentially map fields if
// the library changes?
pub async fn post_trade(
    State(state): State<Arc<AppState>>,
    trade_params: Json<TradeParams>,
) -> Result<(), AppError> {
    state.node.trade(&trade_params.0).await.map_err(|e| {
        AppError::InternalServerError(format!("Could not handle trade request: {e:#}"))
    })?;

    Ok(())
}

pub async fn post_register(
    State(state): State<Arc<AppState>>,
    params: Json<RegisterParams>,
) -> Result<(), AppError> {
    let register_params = params.0;
    if !register_params.is_valid() {
        return Err(AppError::BadRequest(format!(
            "Invalid registration parameters: {register_params:?}"
        )));
    }
    tracing::info!(?register_params, "Registered new user");

    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    user::insert(&mut conn, register_params.into())
        .map_err(|e| AppError::InternalServerError(format!("Could not insert user: {e:#}")))?;

    Ok(())
}
