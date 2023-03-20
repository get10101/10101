use crate::node::Node;
use crate::orderbook::routes::delete_order;
use crate::orderbook::routes::get_order;
use crate::orderbook::routes::get_orders;
use crate::orderbook::routes::post_order;
use crate::orderbook::routes::put_order;
use crate::orderbook::routes::websocket_handler;
use crate::AppError;
use axum::extract::Path;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::secp256k1::PublicKey;
use coordinator_commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::ChannelDetails;
use ln_dlc_node::DlcChannelDetails;
use orderbook_commons::OrderbookMsg;
use serde::Deserialize;
use serde::Serialize;
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
        .route("/api/newaddress", get(get_new_address))
        .route("/api/node", get(get_node_info))
        .route("/api/balance", get(get_balance))
        .route("/api/invoice", get(get_invoice))
        .route("/api/orderbook/orders", get(get_orders).post(post_order))
        .route(
            "/api/orderbook/orders/:order_id",
            get(get_order).put(put_order).delete(delete_order),
        )
        .route("/api/orderbook/websocket", get(websocket_handler))
        .route("/api/trade", post(post_trade))
        .route("/api/channels", get(list_channels))
        .route("/api/dlc_channels", get(list_dlc_channels))
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

pub async fn get_node_info(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<NodeInfo>, AppError> {
    let node_info = app_state.node.inner.info;
    Ok(Json(node_info))
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

pub async fn list_channels(State(state): State<Arc<AppState>>) -> Json<Vec<ChannelDetails>> {
    let channels = state
        .node
        .inner
        .list_channels()
        .into_iter()
        .map(ChannelDetails::from)
        .collect::<Vec<_>>();

    Json(channels)
}

pub async fn list_dlc_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DlcChannelDetails>>, AppError> {
    let dlc_channels =
        state.node.inner.list_dlc_channels().map_err(|e| {
            AppError::InternalServerError(format!("Failed to get new address: {e:#}"))
        })?;

    let dlc_channels = dlc_channels
        .into_iter()
        .map(DlcChannelDetails::from)
        .collect::<Vec<_>>();

    Ok(Json(dlc_channels))
}
