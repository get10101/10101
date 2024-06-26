use crate::check_version::check_version;
use crate::db;
use crate::orderbook;
use crate::orderbook::db::orders;
use crate::orderbook::trading::NewOrderMessage;
use crate::orderbook::websocket::websocket_connection;
use crate::routes::AppState;
use crate::AppError;
use anyhow::anyhow;
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
use tokio::sync::broadcast::Sender;
use tokio::task::spawn_blocking;
use tracing::instrument;
use uuid::Uuid;
use xxi_node::commons;
use xxi_node::commons::Message;
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

    // TODO(holzeis): We should add a similar check eventually for limit orders (makers).
    if let NewOrder::Market(new_order) = &new_order {
        let mut conn = state
            .pool
            .get()
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        check_version(&mut conn, &new_order.trader_id)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
    }

    let settings = state.settings.read().await;

    if let NewOrder::Limit(new_order) = &new_order {
        if settings.whitelist_enabled && !settings.whitelisted_makers.contains(&new_order.trader_id)
        {
            tracing::warn!(
                trader_id = %new_order.trader_id,
                "Trader tried to post limit order but was not whitelisted"
            );
            return Err(AppError::Unauthorized);
        }

        if new_order.price == Decimal::ZERO {
            return Err(AppError::BadRequest(
                "Limit orders with zero price are not allowed".to_string(),
            ));
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

    let pool = state.pool.clone();
    let new_order = new_order.clone();
    let order = spawn_blocking(move || {
        let mut conn = pool.get()?;

        let order = match new_order {
            NewOrder::Market(o) => {
                orders::insert_market_order(&mut conn, o.clone(), OrderReason::Manual)
            }
            NewOrder::Limit(o) => orders::insert_limit_order(&mut conn, o, OrderReason::Manual),
        }
        .map_err(|e| anyhow!(e))
        .context("Failed to insert new order into DB")?;

        anyhow::Ok(order)
    })
    .await
    .expect("task to complete")
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // FIXME(holzeis): We shouldn't blindly trust the user about the coordinator reserve. Note, we
    // already ignore the trader reserve parameter when the channel is externally funded.
    let message = NewOrderMessage {
        order,
        channel_opening_params: new_order_request.channel_opening_params.map(|params| {
            crate::ChannelOpeningParams {
                trader_reserve: params.trader_reserve,
                coordinator_reserve: params.coordinator_reserve,
                external_funding,
            }
        }),
        order_reason: OrderReason::Manual,
    };

    state.trading_sender.send(message).await.map_err(|e| {
        AppError::InternalServerError(format!("Failed to send new order message: {e:#}"))
    })?;

    Ok(())
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

#[instrument(skip_all, err(Debug))]
pub async fn delete_order(
    Path(order_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Order>, AppError> {
    let mut conn = get_db_connection(&state)?;
    let order = orderbook::db::orders::delete(&mut conn, order_id)
        .map_err(|e| AppError::InternalServerError(format!("Failed to delete order: {e:#}")))?;
    let sender = state.tx_orderbook_feed.clone();
    update_pricefeed(Message::DeleteOrder(order_id), sender);

    Ok(Json(order))
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}
