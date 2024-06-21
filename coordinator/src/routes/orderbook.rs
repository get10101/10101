use crate::check_version::check_version;
use crate::db;
use crate::orderbook;
use crate::orderbook::trading::OrderbookMessage;
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
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::instrument;
use uuid::Uuid;
use xxi_node::commons;
use xxi_node::commons::NewOrder;
use xxi_node::commons::NewOrderRequest;
use xxi_node::commons::Order;
use xxi_node::commons::OrderReason;
use xxi_node::commons::OrderState;
use xxi_node::commons::OrderType;

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
) -> Result<(), AppError> {
    new_order_request
        .verify(&state.secp)
        .map_err(|_| AppError::Unauthorized)?;

    let new_order = new_order_request.value;
    let order_id = new_order.id();

    match new_order {
        NewOrder::Market(new_order) => {
            spawn_blocking({
                let pool = state.pool.clone();
                move || {
                    let mut conn = pool
                        .get()
                        .context("Could not acquire database connection")?;
                    // TODO(holzeis): We should add a similar check eventually for limit orders
                    // (makers).
                    check_version(&mut conn, &new_order.trader_id)
                }
            })
            .await
            .expect("task to finish")
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        }
        NewOrder::Limit(new_order) => {
            if new_order.price == Decimal::ZERO {
                return Err(AppError::BadRequest(
                    "Limit orders with zero price are not allowed".to_string(),
                ));
            }

            let (whitelist_enabled, whitelisted_makers) = {
                let settings = state.settings.read().await;
                (
                    settings.whitelist_enabled,
                    settings.whitelisted_makers.clone(),
                )
            };
            if whitelist_enabled && !whitelisted_makers.contains(&new_order.trader_id) {
                tracing::warn!(
                    trader_id = %new_order.trader_id,
                    "Trader tried to post limit order but was not whitelisted"
                );
                return Err(AppError::Unauthorized);
            }
        }
    }

    let pool = state.pool.clone();
    let external_funding = match new_order_request
        .channel_opening_params
        .clone()
        .and_then(|c| c.pre_image)
    {
        Some(pre_image_str) => {
            let pre_image =
                commons::PreImage::from_url_safe_encoded_pre_image(pre_image_str.as_str())
                    .map_err(|_| AppError::BadRequest("Invalid pre_image provided".to_string()))?;

            tracing::debug!(
                pre_image_str,
                hash = pre_image.hash,
                "Received pre-image, updating records"
            );

            let inner_hash = pre_image.hash.clone();
            let funding_amount = spawn_blocking(move || {
                let mut conn = pool.get()?;

                let amount = db::hodl_invoice::update_hodl_invoice_to_accepted(
                    &mut conn,
                    inner_hash.as_str(),
                    pre_image_str.as_str(),
                    order_id,
                )?;

                anyhow::Ok(amount)
            })
            .await
            .expect("task to complete")
            .map_err(|e| AppError::BadRequest(format!("Invalid pre_image provided: {e:#}")))?;

            // we have received funding via lightning and can now open the channel with funding
            // only from the coordinator
            Some(funding_amount)
        }
        None => None,
    };

    // FIXME(holzeis): We shouldn't blindly trust the user about the coordinator reserve. Note,
    // we already ignore the trader reserve parameter when the channel is externally
    // funded.
    if let Some(channel_opening_params) = new_order_request.channel_opening_params {
        spawn_blocking({
            let pool = state.pool.clone();
            move || {
                let mut conn = pool.get()?;
                db::channel_opening_params::insert(
                    &mut conn,
                    order_id,
                    crate::ChannelOpeningParams {
                        trader_reserve: channel_opening_params.trader_reserve,
                        coordinator_reserve: channel_opening_params.coordinator_reserve,
                        external_funding,
                    },
                )?;
                anyhow::Ok(())
            }
        })
        .await
        .expect("task to complete")
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to store channel opening params: {e:#}"))
        })?;
    }

    let message = OrderbookMessage::NewOrder {
        new_order,
        order_reason: OrderReason::Manual,
    };

    state.orderbook_sender.send(message).await.map_err(|e| {
        AppError::InternalServerError(format!("Failed to send new order message: {e:#}"))
    })?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
pub async fn delete_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .orderbook_sender
        .send(OrderbookMessage::DeleteOrder(order_id))
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to send delete order message: {e:#}"))
        })?;

    Ok(())
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}
