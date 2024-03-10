use crate::check_version::check_version;
use crate::orderbook;
use crate::orderbook::trading::NewOrderMessage;
use crate::orderbook::trading::TradingError;
use crate::orderbook::websocket::websocket_connection;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use anyhow::Result;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::Path;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use commons::Message;
use commons::NewOrderRequest;
use commons::Order;
use commons::OrderReason;
use commons::OrderState;
use commons::OrderType;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

#[instrument(skip_all, err(Debug))]
fn get_db_connection(
    state: &Arc<AppState>,
) -> Result<PooledConnection<ConnectionManager<PgConnection>>, AppError> {
    state
        .pool
        .clone()
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Failed to get db access: {e:#}")))
}

#[instrument(skip_all, err(Debug))]
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
pub async fn get_orders(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Order>>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let orders =
        orderbook::db::orders::get_all_orders(&mut conn, OrderType::Limit, OrderState::Open, true)
            .map_err(|e| AppError::InternalServerError(format!("Failed to load order: {e:#}")))?;

    Ok(Json(orders))
}

#[instrument(skip_all, err(Debug))]
pub async fn post_order(
    State(state): State<Arc<AppState>>,
    Json(new_order_request): Json<NewOrderRequest>,
) -> Result<Json<Order>, AppError> {
    new_order_request
        .verify(&state.secp)
        .map_err(|_| AppError::Unauthorized)?;

    let new_order = new_order_request.value;

    // TODO(holzeis): We should add a similar check eventually for limit orders (makers).
    if new_order.order_type == OrderType::Market {
        let mut conn = state
            .pool
            .get()
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        check_version(&mut conn, &new_order.trader_id)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
    }

    let settings = state.settings.read().await;
    if new_order.order_type == OrderType::Limit
        && settings.whitelist_enabled
        && !settings.whitelisted_makers.contains(&new_order.trader_id)
    {
        tracing::warn!(
            trader_id = %new_order.trader_id,
            "Trader tried to post limit order but was not whitelisted"
        );
        return Err(AppError::Unauthorized);
    }

    let (sender, mut receiver) = mpsc::channel::<Result<Order>>(1);

    let message = NewOrderMessage {
        new_order,
        order_reason: OrderReason::Manual,
        sender,
    };
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

fn update_pricefeed(pricefeed_msg: Message, sender: Sender<Message>) {
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

#[instrument(skip_all, err(Debug))]
pub async fn put_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(updated_order): Json<UpdateOrder>,
) -> Result<Json<Order>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let order = orderbook::db::orders::set_is_taken(&mut conn, order_id, updated_order.taken)
        .map_err(|e| AppError::InternalServerError(format!("Failed to update order: {e:#}")))?;
    let sender = state.tx_price_feed.clone();
    update_pricefeed(Message::Update(order.clone()), sender);

    Ok(Json(order))
}

#[instrument(skip_all, err(Debug))]
pub async fn delete_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Order>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let order = orderbook::db::orders::delete(&mut conn, order_id)
        .map_err(|e| AppError::InternalServerError(format!("Failed to delete order: {e:#}")))?;
    let sender = state.tx_price_feed.clone();
    update_pricefeed(Message::Update(order.clone()), sender);

    Ok(Json(order))
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}
