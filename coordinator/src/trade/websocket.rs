use crate::db;
use crate::position::models::Position;
use crate::routes::AppState;
use axum::extract::ws::Message as WebsocketMessage;
use axum::extract::ws::WebSocket;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use futures::SinkExt;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use trade::Direction;
use xxi_node::commons::create_sign_message;
use xxi_node::commons::PositionMessage;
use xxi_node::commons::PositionMessageRequest;
use xxi_node::commons::AUTH_SIGN_MESSAGE;

#[derive(Clone)]
pub enum InternalPositionUpdateMessage {
    NewTrade {
        /// As seen from the coordinator, i.e. if quantity is < 0 then coordinator is short, if >
        /// 0, then coordinator is long
        quantity: f32,
        average_entry_price: f32,
    },
}

const WEBSOCKET_SEND_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending messages).
pub async fn websocket_connection(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let mut feed = state.tx_position_feed.subscribe();

    let (local_sender, mut local_receiver) = mpsc::channel::<PositionMessage>(100);

    let mut local_recv_task = tokio::spawn(async move {
        while let Some(local_msg) = local_receiver.recv().await {
            match serde_json::to_string(&local_msg) {
                Ok(msg) => {
                    if let Err(err) = tokio::time::timeout(
                        WEBSOCKET_SEND_TIMEOUT,
                        sender.send(WebsocketMessage::Text(msg.clone())),
                    )
                    .await
                    {
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
        let pool = state.pool.clone();
        tokio::spawn(async move {
            loop {
                match feed.recv().await.clone() {
                    Ok(position_update) => match position_update {
                        InternalPositionUpdateMessage::NewTrade {
                            quantity,
                            average_entry_price,
                        } => {
                            if let Err(error) = {
                                let mut conn = match pool.get() {
                                    Ok(conn) => conn,
                                    Err(err) => {
                                        tracing::error!(
                                            "Could not get connection to db pool {err:#}"
                                        );
                                        return;
                                    }
                                };

                                let (total_average_entry_price, total_quantity) =
                                    calculate_position_stats(&mut conn);
                                local_sender.send(PositionMessage::NewTrade {
                                    total_quantity,
                                    total_average_entry_price,
                                    new_trade_quantity: quantity,
                                    new_trade_average_entry_price: average_entry_price,
                                })
                            }
                            .await
                            {
                                tracing::error!("Could not send message {error:#}");
                                return;
                            }
                        }
                    },
                    Err(RecvError::Closed) => {
                        tracing::error!("position feed sender died! Channel closed.");
                        break;
                    }
                    Err(RecvError::Lagged(skip)) => tracing::warn!(%skip,
                        "Lagging behind on position feed."
                    ),
                }
            }
        })
    };

    // Spawn a task that takes messages from the websocket
    let local_sender = local_sender.clone();
    let pool = state.pool.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(WebsocketMessage::Text(text))) = receiver.next().await {
            match serde_json::from_str(text.as_str()) {
                Ok(PositionMessageRequest::Authenticate { signature }) => {
                    let msg = create_sign_message(AUTH_SIGN_MESSAGE.to_vec());
                    // TODO(bonomat): in the future we could add authorization as well to only allow
                    // particular pubkeys get updates
                    let user_id = signature.pubkey;
                    let signature = signature.signature;

                    let mut conn = match pool.get() {
                        Ok(conn) => conn,
                        Err(err) => {
                            tracing::error!("Could not get connection to db pool {err:#}");
                            return;
                        }
                    };

                    match state.secp.verify_ecdsa(&msg, &signature, &user_id) {
                        Ok(_) => {
                            if let Err(e) = local_sender.send(PositionMessage::Authenticated).await
                            {
                                tracing::error!(%user_id, "Could not respond to user {e:#}");
                                return;
                            }

                            let (average_entry_price, total_quantity) =
                                calculate_position_stats(&mut conn);

                            if let Err(e) = local_sender
                                .send(PositionMessage::CurrentPosition {
                                    quantity: total_quantity,
                                    average_entry_price,
                                })
                                .await
                            {
                                tracing::error!(%user_id, "Failed to send all open positions to user {e:#}");
                            }
                        }
                        Err(err) => {
                            if let Err(er) = local_sender
                                .send(PositionMessage::InvalidAuthentication(format!(
                                    "Could not authenticate {err:#}"
                                )))
                                .await
                            {
                                tracing::error!(
                                    %user_id, "Failed to notify user about invalid authentication: {er:#}"
                                );
                                return;
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::trace!("Could not deserialize msg: {text} {err:#}");
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

/// Calculates position stats and returns as a tuple (`average_entry_price`,`total_quantity`)
fn calculate_position_stats(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> (f32, f32) {
    let positions = db::positions::Position::get_all_open_positions(conn).unwrap_or_default();

    let average_entry_price = average_entry_price(&positions);
    let total_quantity = positions
        .iter()
        .map(|pos| {
            if pos.trader_direction == Direction::Short {
                pos.quantity
            } else {
                // we want to see the quantity as seen from the coordinator
                pos.quantity * -1.0
            }
        })
        .sum();
    (average_entry_price, total_quantity)
}

/// calculates the average execution price for inverse contracts
///
/// The average execution price follows a simple formula:
/// `total_order_quantity / (quantity_position_0 / execution_price_position_0 + quantity_position_1
/// / execution_price_position_1 )`
pub fn average_entry_price(positions: &[Position]) -> f32 {
    if positions.is_empty() {
        return 0.0;
    }
    if positions.len() == 1 {
        return positions
            .first()
            .expect("to be exactly one")
            .average_entry_price;
    }
    let sum_quantity = positions.iter().fold(0.0, |acc, m| acc + m.quantity);

    let nominal_prices = positions
        .iter()
        .fold(0.0, |acc, m| acc + (m.quantity / m.average_entry_price));

    sum_quantity / nominal_prices
}
