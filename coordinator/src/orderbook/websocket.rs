use crate::message::NewUserMessage;
use crate::orderbook;
use crate::orderbook::db::orders;
use crate::routes::AppState;
use axum::extract::ws::Message as WebsocketMessage;
use axum::extract::ws::WebSocket;
use futures::SinkExt;
use futures::StreamExt;
use orderbook_commons::create_sign_message;
use orderbook_commons::Message;
use orderbook_commons::OrderbookRequest;
use orderbook_commons::Signature;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const WEBSOCKET_SEND_TIMEOUT: Duration = Duration::from_secs(5);

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending messages).
pub async fn websocket_connection(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // We subscribe *before* sending the "joined" message, so that we will also
    // display it to our client.
    let mut price_feed = state.tx_price_feed.subscribe();

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
    if let Ok(msg) = serde_json::to_string(&Message::AllOrders(orders)) {
        let _ = sender.send(WebsocketMessage::Text(msg)).await;
    }

    let (local_sender, mut local_receiver) = mpsc::channel::<Message>(100);

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
        tokio::spawn(async move {
            while let Ok(st) = price_feed.recv().await {
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
        while let Some(Ok(WebsocketMessage::Text(text))) = receiver.next().await {
            match serde_json::from_str(text.as_str()) {
                Ok(OrderbookRequest::LimitOrderFilledMatches { trader_id }) => {
                    let mut conn = match state.pool.get() {
                        Ok(conn) => conn,
                        Err(e) => {
                            tracing::error!(
                                %trader_id,
                                "Failed to get DB pool connection to get limit order filled matches: {e:#}"
                            );
                            continue;
                        }
                    };

                    let matches =
                        match orders::get_all_limit_order_filled_matches(&mut conn, trader_id) {
                            Ok(orders) => orders,
                            Err(e) => {
                                tracing::error!(
                                    %trader_id,
                                    "Failed to get limit order filled matches from DB: {e:#}"
                                );
                                continue;
                            }
                        };

                    if let Err(e) = local_sender
                        .send(Message::LimitOrderFilledMatches { trader_id, matches })
                        .await
                    {
                        tracing::error!(
                            %trader_id,
                            "Failed to notify user about limit order filled matches: {e:#}"
                        );
                    }
                }
                Ok(OrderbookRequest::Authenticate(Signature { signature, pubkey })) => {
                    let msg = create_sign_message();
                    match signature.verify(&msg, &pubkey) {
                        Ok(_) => {
                            if let Err(e) = local_sender.send(Message::Authenticated).await {
                                tracing::error!("Could not respond to user {e:#}");
                                return;
                            }

                            let message = NewUserMessage {
                                new_user: pubkey,
                                sender: local_sender.clone(),
                            };
                            if let Err(e) = state.tx_user_feed.send(message) {
                                tracing::error!("Could not send trading message. Error: {e:#}");
                            }
                        }
                        Err(err) => {
                            if let Err(er) = local_sender
                                .send(Message::InvalidAuthentication(format!(
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
