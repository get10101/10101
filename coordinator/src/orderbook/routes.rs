use crate::orderbook;
use crate::orderbook::trading::NewOrderMessage;
use crate::orderbook::trading::TradingError;
use crate::orderbook::trading::TradingMessage;
use crate::orderbook::websocket::websocket_connection;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use anyhow::Result;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use orderbook_commons::NewOrder;
use orderbook_commons::Order;
use orderbook_commons::OrderReason;
use orderbook_commons::OrderbookMsg;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AllOrdersParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    show_expired: Option<bool>,
}

/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

pub async fn get_orders(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AllOrdersParams>,
) -> Result<Json<Vec<Order>>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let show_expired = params.show_expired.unwrap_or_default();
    let order = orderbook::db::orders::all(&mut conn, show_expired)
        .map_err(|e| AppError::InternalServerError(format!("Failed to load all orders: {e:#}")))?;

    Ok(Json(order))
}

fn get_db_connection(
    state: &Arc<AppState>,
) -> Result<PooledConnection<ConnectionManager<PgConnection>>, AppError> {
    state
        .pool
        .clone()
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get db access: {e:#}")))
}

pub async fn get_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Order>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let order = orderbook::db::orders::get_with_id(&mut conn, order_id)
        .map_err(|e| AppError::InternalServerError(format!("Failed to load order: {e:#}")))?
        .context(format!("Order not found {order_id}"))
        .map_err(|e| AppError::BadRequest(format!("{e:#}")))?;

    Ok(Json(order))
}

#[instrument(skip_all, err(Debug))]
pub async fn post_order(
    State(state): State<Arc<AppState>>,
    Json(new_order): Json<NewOrder>,
) -> Result<Json<Order>, AppError> {
    let (sender, mut receiver) = mpsc::channel::<Result<Order>>(1);

    let message = TradingMessage::NewOrder(NewOrderMessage {
        new_order,
        order_reason: OrderReason::Manual,
        sender,
    });
    state.trading_sender.send(message).await.map_err(|e| {
        AppError::InternalServerError(format!("Failed to send new order message: {e:#}"))
    })?;

    let result = receiver
        .recv()
        .await
        .context("Failed to receive response from trading sender")
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    let order = result.map_err(|e| match e.downcast_ref() {
        Some(TradingError::InvalidOrder(reason)) => AppError::InvalidOrder(reason.to_string()),
        Some(TradingError::NoMatchFound(message)) => AppError::NoMatchFound(message.to_string()),
        _ => AppError::InternalServerError(format!("Failed to post order. Error: {e:#}")),
    })?;

    Ok(Json(order))
}

fn update_pricefeed(pricefeed_msg: OrderbookMsg, sender: Sender<OrderbookMsg>) {
    match sender.send(pricefeed_msg) {
        Ok(_) => {
            tracing::trace!("Pricefeed updated")
        }
        Err(error) => {
            tracing::warn!("Could not update pricefeed due to '{error}'")
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct UpdateOrder {
    pub taken: bool,
}

pub async fn put_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(updated_order): Json<UpdateOrder>,
) -> Result<Json<Order>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let order = orderbook::db::orders::set_is_taken(&mut conn, order_id, updated_order.taken)
        .map_err(|e| AppError::InternalServerError(format!("Failed to update order: {e:#}")))?;
    let sender = state.tx_price_feed.clone();
    update_pricefeed(OrderbookMsg::Update(order.clone()), sender);

    Ok(Json(order))
}

pub async fn delete_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<usize>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let deleted = orderbook::db::orders::delete_with_id(&mut conn, order_id)
        .map_err(|e| AppError::InternalServerError(format!("Failed to delete order: {e:#}")))?;
    if deleted > 0 {
        let sender = state.tx_price_feed.clone();
        update_pricefeed(OrderbookMsg::DeleteOrder(order_id), sender);
    }

    Ok(Json(deleted))
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}
