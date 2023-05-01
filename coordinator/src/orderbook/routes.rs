use crate::orderbook;
use crate::orderbook::db;
use crate::orderbook::db::orders;
use crate::orderbook::trading::match_order;
use crate::orderbook::trading::notify_traders;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use anyhow::Result;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use futures::SinkExt;
use futures::StreamExt;
use orderbook_commons::create_sign_message;
use orderbook_commons::FilledWith;
use orderbook_commons::NewOrder;
use orderbook_commons::Order;
use orderbook_commons::OrderType;
use orderbook_commons::OrderbookMsg;
use orderbook_commons::OrderbookRequest;
use orderbook_commons::Signature;
use rust_decimal::Decimal;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
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

#[derive(Clone)]
pub struct MatchParams {
    pub taker_matches: TraderMatchParams,
    pub makers_matches: Vec<TraderMatchParams>,
}

#[derive(Clone)]
pub struct TraderMatchParams {
    pub trader_id: PublicKey,
    pub filled_with: FilledWith,
}

pub async fn post_order(
    State(state): State<Arc<AppState>>,
    Json(new_order): Json<NewOrder>,
) -> Result<Json<Order>, AppError> {
    if new_order.order_type == OrderType::Limit && new_order.price == Decimal::ZERO {
        return Err(AppError::InvalidOrder(
            "Limit order with zero price are not allowed".to_string(),
        ));
    }

    let mut conn = get_db_connection(&state)?;
    let order = orderbook::db::orders::insert(&mut conn, new_order.clone()).map_err(|e| {
        AppError::InternalServerError(format!("Failed to insert new order into db: {e:#}"))
    })?;

    if new_order.order_type == OrderType::Limit {
        // we only tell everyone about new limit orders
        let sender = state.tx_pricefeed.clone();
        update_pricefeed(OrderbookMsg::NewOrder(order.clone()), sender);
        return Ok(Json(order));
    }

    let all_non_expired_orders = orders::all_by_direction_and_type(
        &mut conn,
        order.direction.opposite(),
        OrderType::Limit,
        false,
        true,
    )
    .map_err(|e| AppError::InternalServerError(format!("Failed to load all orders: {e:#}")))?;
    let matched_orders = match_order(order.clone(), all_non_expired_orders)
        .map_err(|e| AppError::InternalServerError(format!("Failed to match order: {e:#}")))?;

    let authenticated_users = state.authenticated_users.lock().await;
    match matched_orders {
        Some(matched_orders) => {
            let mut orders_to_set_taken = vec![matched_orders.taker_matches.filled_with.order_id];
            let mut order_ids = matched_orders
                .taker_matches
                .filled_with
                .matches
                .iter()
                .map(|m| m.order_id)
                .collect();

            orders_to_set_taken.append(&mut order_ids);

            notify_traders(matched_orders, authenticated_users.clone()).await;

            for order_id in orders_to_set_taken {
                if let Err(err) = db::orders::set_is_taken(&mut conn, order_id, true) {
                    let order_id = order_id.to_string();
                    tracing::error!(order_id, "Could not set order to taken {err:#}");
                }
            }
        }
        None => return Err(AppError::NoMatchFound("Could not match order".to_string())),
    };

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
    let sender = state.tx_pricefeed.clone();
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
        let sender = state.tx_pricefeed.clone();
        update_pricefeed(OrderbookMsg::DeleteOrder(order_id), sender);
    }

    Ok(Json(deleted))
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending messages).
async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // We subscribe *before* sending the "joined" message, so that we will also
    // display it to our client.
    let mut rx = state.tx_pricefeed.subscribe();

    let mut conn = match state.pool.clone().get() {
        Ok(conn) => conn,
        Err(err) => {
            tracing::error!("Could not get connection to db pool {err:#}");
            return;
        }
    };

    let orders = match orderbook::db::orders::all(&mut conn, false) {
        Ok(orders) => orders,
        Err(error) => {
            tracing::error!("Could not load all orders from db {error:#}");
            return;
        }
    };

    // Now send the "all orders" to the new client.
    if let Ok(msg) = serde_json::to_string(&OrderbookMsg::AllOrders(orders)) {
        let _ = sender.send(Message::Text(msg)).await;
    }

    let (local_sender, mut local_receiver) = mpsc::channel::<OrderbookMsg>(100);

    let mut local_recv_task = tokio::spawn(async move {
        while let Some(local_msg) = local_receiver.recv().await {
            match serde_json::to_string(&local_msg) {
                Ok(msg) => {
                    if let Err(err) = sender.send(Message::Text(msg.clone())).await {
                        tracing::error!("Could not forward message {msg} : {err:#}");
                        return;
                    }
                }
                Err(error) => {
                    tracing::warn!("Could not deserialize message {error:#}");
                }
            }
        }
    });

    // Spawn the first task that will receive broadcast messages and send
    // messages over the websocket to our client.
    let mut send_task = {
        let local_sender = local_sender.clone();
        tokio::spawn(async move {
            while let Ok(st) = rx.recv().await {
                if let Err(error) = local_sender.send(st).await {
                    tracing::error!("Could not send message {error:#}");
                    return;
                }
            }
        })
    };

    // Spawn a task that takes messages from the websocket
    let local_sender = local_sender.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            match serde_json::from_str(text.as_str()) {
                Ok(OrderbookRequest::Authenticate(Signature { signature, pubkey })) => {
                    let msg = create_sign_message();
                    match signature.verify(&msg, &pubkey) {
                        Ok(_) => {
                            if let Err(e) = local_sender.send(OrderbookMsg::Authenticated).await {
                                tracing::error!("Could not respond to user {e:#}");
                                return;
                            }

                            let mut authenticated_users = state.authenticated_users.lock().await;
                            authenticated_users.insert(pubkey, local_sender.clone());
                        }
                        Err(err) => {
                            if let Err(er) = local_sender
                                .send(OrderbookMsg::InvalidAuthentication(format!(
                                    "Could not authenticate {err:#}"
                                )))
                                .await
                            {
                                tracing::error!(
                                    "Failed to notify user about invalid authentication: {er:#}"
                                );
                                return;
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::trace!("Could not derserialize msg: {text} {err:#}");
                }
            }
        }
    });

    // If any one of the tasks run to completion, we abort the other.
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
            local_recv_task.abort()
        },
        _ = (&mut recv_task) => {
            send_task.abort();
            local_recv_task.abort()
        },
        _ = (&mut local_recv_task) => {
            recv_task.abort();
            send_task.abort();
        },
    };
}
