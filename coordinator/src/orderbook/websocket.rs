use crate::db;
use crate::db::funding_fee_events;
use crate::db::user;
use crate::message::NewUserMessage;
use crate::orderbook::db::orders;
use crate::orderbook::trading::NewOrderMessage;
use crate::referrals;
use crate::routes::AppState;
use anyhow::bail;
use anyhow::Result;
use axum::extract::ws::Message as WebsocketMessage;
use axum::extract::ws::WebSocket;
use bitcoin::secp256k1::PublicKey;
use futures::SinkExt;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use uuid::Uuid;
use xxi_node::commons::create_sign_message;
use xxi_node::commons::Message;
use xxi_node::commons::NewLimitOrder;
use xxi_node::commons::OrderReason;
use xxi_node::commons::OrderbookRequest;
use xxi_node::commons::ReferralStatus;
use xxi_node::commons::TenTenOneConfig;
use xxi_node::commons::AUTH_SIGN_MESSAGE;

const WEBSOCKET_SEND_TIMEOUT: Duration = Duration::from_secs(5);

async fn handle_insert_order(
    state: Arc<AppState>,
    trader_id: PublicKey,
    order: NewLimitOrder,
) -> Result<()> {
    if order.trader_id != trader_id {
        bail!("Maker {trader_id} tried to trade on behalf of someone else: {order:?}");
    }

    tracing::trace!(?order, "Inserting order");

    let order = spawn_blocking({
        let mut conn = state.pool.clone().get()?;
        move || {
            let order = orders::insert_limit_order(&mut conn, order, OrderReason::Manual)?;

            anyhow::Ok(order)
        }
    })
    .await??;

    let _ = state
        .trading_sender
        .send(NewOrderMessage {
            order,
            channel_opening_params: None,
            order_reason: OrderReason::Manual,
        })
        .await;

    Ok(())
}

async fn handle_delete_order(
    state: Arc<AppState>,
    trader_id: PublicKey,
    order_id: Uuid,
) -> Result<()> {
    tracing::trace!(%order_id, "Deleting order");

    spawn_blocking({
        let mut conn = state.pool.clone().get()?;
        move || {
            orders::delete_trader_order(&mut conn, order_id, trader_id)?;

            anyhow::Ok(())
        }
    })
    .await??;

    let _ = state.tx_orderbook_feed.send(Message::DeleteOrder(order_id));

    Ok(())
}

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending messages).
pub async fn websocket_connection(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // We subscribe *before* sending the "joined" message, so that we will also
    // display it to our client.
    let mut price_feed = state.tx_orderbook_feed.subscribe();

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
            loop {
                match price_feed.recv().await {
                    Ok(st) => {
                        if let Err(error) = local_sender.send(st).await {
                            tracing::error!("Could not send message {error:#}");
                            return;
                        }
                    }
                    Err(RecvError::Closed) => {
                        tracing::error!("price feed sender died! Channel closed.");
                        break;
                    }
                    Err(RecvError::Lagged(skip)) => tracing::warn!(%skip,
                        "Lagging behind on price feed."
                    ),
                }
            }
        })
    };

    // Spawn a task that takes messages from the websocket
    let local_sender = local_sender.clone();
    let mut recv_task = tokio::spawn(async move {
        let mut whitelisted_maker = Option::<PublicKey>::None;

        while let Some(Ok(WebsocketMessage::Text(text))) = receiver.next().await {
            match serde_json::from_str(text.as_str()) {
                Ok(OrderbookRequest::InsertOrder(order)) => {
                    let order_id = order.id;

                    match whitelisted_maker {
                        Some(authenticated_trader_id) => {
                            if let Err(e) =
                                handle_insert_order(state.clone(), authenticated_trader_id, order)
                                    .await
                            {
                                tracing::error!(%order_id, "Failed to insert order: {e:#}");
                                // TODO: Send error to peer.
                            }
                        }
                        None => {
                            tracing::error!(
                                ?order,
                                "Failed to insert order: maker not yet authenticated"
                            );
                        }
                    }
                }
                Ok(OrderbookRequest::DeleteOrder(order_id)) => {
                    match whitelisted_maker {
                        Some(authenticated_trader_id) => {
                            if let Err(e) = handle_delete_order(
                                state.clone(),
                                authenticated_trader_id,
                                order_id,
                            )
                            .await
                            {
                                tracing::error!(%order_id, "Failed to delete order: {e:#}");
                                // TODO: Send error to peer.
                            }
                        }
                        None => {
                            tracing::error!(
                                %order_id,
                                "Failed to delete order: maker not yet authenticated"
                            );
                        }
                    }
                }
                Ok(OrderbookRequest::Authenticate {
                    fcm_token,
                    version,
                    os,
                    signature,
                }) => {
                    let msg = create_sign_message(AUTH_SIGN_MESSAGE.to_vec());
                    let trader_id = signature.pubkey;
                    let signature = signature.signature;

                    let mut conn = match state.pool.clone().get() {
                        Ok(conn) => conn,
                        Err(err) => {
                            tracing::error!("Could not get connection to db pool {err:#}");
                            return;
                        }
                    };

                    match state.secp.verify_ecdsa(&msg, &signature, &trader_id) {
                        Ok(_) => {
                            let liquidity_options =
                                db::liquidity_options::get_all(&mut conn).unwrap_or_default();

                            let (min_quantity, maintenance_margin_rate, order_matching_fee_rate) = {
                                let settings = state.settings.read().await;
                                (
                                    settings.min_quantity,
                                    settings.maintenance_margin_rate,
                                    settings.order_matching_fee_rate,
                                )
                            };

                            let referral_status = referrals::update_referral_status_for_user(
                                &mut conn,
                                trader_id.to_string(),
                            )
                            .unwrap_or(ReferralStatus::new(trader_id));
                            if let Err(e) = local_sender
                                .send(Message::Authenticated(TenTenOneConfig {
                                    liquidity_options,
                                    min_quantity,
                                    maintenance_margin_rate,
                                    order_matching_fee_rate,
                                    referral_status,
                                }))
                                .await
                            {
                                tracing::error!(%trader_id, "Could not respond to user {e:#}");
                                return;
                            }

                            let orders = orders::all_limit_orders(&mut conn).unwrap_or_default();
                            if let Err(e) = local_sender.send(Message::AllOrders(orders)).await {
                                tracing::error!(%trader_id, "Failed to send all orders to user {e:#}");
                            }

                            // Send over all the funding fee events that the trader may have missed
                            // whilst they were offline.
                            match funding_fee_events::get_for_active_trader_positions(
                                &mut conn, trader_id,
                            ) {
                                Ok(funding_fee_events) => {
                                    if let Err(e) = local_sender
                                        .send(Message::AllFundingFeeEvents(funding_fee_events))
                                        .await
                                    {
                                        tracing::error!(
                                            %trader_id,
                                            "Failed to send funding fee events \
                                             for active positions: {e}"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        %trader_id,
                                        "Failed to load funding fee events \
                                         for active positions: {e}"
                                    );
                                }
                            }

                            let token = fcm_token.unwrap_or("unavailable".to_string());
                            if let Err(e) =
                                user::login_user(&mut conn, trader_id, token, version, os)
                            {
                                tracing::error!(%trader_id, "Failed to update logged in user. Error: {e:#}")
                            }

                            let message = NewUserMessage {
                                new_user: trader_id,
                                sender: local_sender.clone(),
                            };

                            tracing::debug!(%trader_id, "New login");

                            // Check if the trader is a whitelisted maker.
                            {
                                let settings = state.settings.read().await;

                                if !settings.whitelist_enabled
                                    || settings.whitelisted_makers.contains(&trader_id)
                                {
                                    whitelisted_maker = Some(trader_id);
                                }
                            }

                            if let Err(e) = state.tx_user_feed.send(message) {
                                tracing::error!(%trader_id, "Could not send new user message. Error: {e:#}");
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
                                    %trader_id, "Failed to notify user about invalid authentication: {er:#}"
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
