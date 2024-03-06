use crate::config;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::health::ServiceStatus;
use crate::ln_dlc;
use crate::state;
use crate::trade::order;
use crate::trade::order::FailureReason;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::SECP256K1;
use commons::best_current_price;
use commons::Message;
use commons::Order;
use commons::OrderbookRequest;
use commons::Prices;
use commons::Signature;
use futures::SinkExt;
use futures::TryStreamExt;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;
use tokio_tungstenite_wasm as tungstenite;
use uuid::Uuid;

/// FIXME(holzeis): There is an edge case where the app is still open while we move into the
/// rollover window. If the coordinator restarts while the app remains open in that scenario, the
/// rollover will fail. However the rollover will succeed on the next restart.
/// This could be fixed by only sending the rollover message once the channel is usable with the
/// trader.
const WS_RECONNECT_TIMEOUT: Duration = Duration::from_millis(200);

const EXPIRED_ORDER_PRUNING_INTERVAL: Duration = Duration::from_secs(30);

pub fn subscribe(
    secret_key: SecretKey,
    runtime: &Runtime,
    orderbook_status: watch::Sender<ServiceStatus>,
    fcm_token: String,
    tx_websocket: broadcast::Sender<OrderbookRequest>,
) -> Result<()> {
    runtime.spawn(async move {
        let url = format!(
            "ws://{}/api/orderbook/websocket",
            config::get_http_endpoint()
        );

        let pubkey = secret_key.public_key(SECP256K1);
        let authenticate = move |msg| {
            let signature = secret_key.sign_ecdsa(msg);
            Signature { pubkey, signature }
        };

        // Need a Mutex as it's being accessed from websocket stream and pruning task
        let orders = Arc::new(Mutex::new(Vec::<Order>::new()));

        let _prune_expired_orders_task = {
            let orders = orders.clone();
            tokio::spawn(async move {
                loop {
                    {
                        tracing::debug!("Pruning expired orders");
                        let mut orders = orders.lock();
                        let orders_before_pruning = orders.len();
                        *orders = orders
                            .iter()
                            .filter(|order| order.expiry >= OffsetDateTime::now_utc())
                            .cloned()
                            .collect::<Vec<_>>();
                        let orders_after_pruning = orders.len();

                        if orders_after_pruning < orders_before_pruning {
                            let amount_pruned = orders_before_pruning - orders_after_pruning;
                            tracing::debug!(
                                orders_before_pruning,
                                orders_after_pruning,
                                "Pruned {amount_pruned} expired orders"
                            );

                            // Current best price might have changed
                            if let Err(e) =
                                position::handler::price_update(best_current_price(&orders))
                            {
                                tracing::error!(
                                    "Price update from the orderbook failed. Error: {e:#}"
                                );
                            }
                        }
                    }
                    tokio::time::sleep(EXPIRED_ORDER_PRUNING_INTERVAL).await;
                }
            })
        };

        let fcm_token = if fcm_token.is_empty() {
            None
        } else {
            Some(fcm_token)
        };

        let mut round = 1;
        loop {
            let url = url.clone();
            let fcm_token = fcm_token.clone();
            let version = env!("CARGO_PKG_VERSION").to_string();
            match orderbook_client::subscribe_with_authentication(url, authenticate, fcm_token, Some(version))
                .await
            {
                Ok((mut sink, mut stream)) => {
                    if let Err(e) = orderbook_status.send(ServiceStatus::Online) {
                        tracing::warn!("Cannot update orderbook status: {e:#}");
                    };

                    let handle = tokio::spawn({
                        let tx_websocket = tx_websocket.clone();
                        async move {
                            let mut receiver = tx_websocket.subscribe();
                            loop {
                                match receiver.recv().await {
                                    Ok(message) => {
                                        let message = tungstenite::Message::try_from(message).expect("to fit into message");
                                        if let Err(e) = sink.send(message).await {
                                            tracing::error!("Failed to send message on websocket. {e:#}");
                                        }
                                    }
                                    Err(RecvError::Lagged(skip)) => {
                                        tracing::warn!(%skip, "Lagging behind on orderbook requests.");
                                    }
                                    Err(RecvError::Closed) => {
                                        tracing::error!(
                                        "Orderbook requests sender died! Channel closed."
                                    );
                                        break;
                                    }
                                }
                            }
                        }
                    });

                    let mut cached_best_price: Prices = HashMap::new();
                    loop {
                        let msg = match stream.try_next().await {
                            Ok(Some(msg)) => msg,
                            Ok(None) => {
                                tracing::warn!("Orderbook WS stream closed");
                                break;
                            }
                            Err(error) => {
                                tracing::warn!(%error, "Orderbook WS stream closed with error");
                                break;
                            }
                        };

                        if let Err(e) =
                            handle_orderbook_message(orders.clone(), &mut cached_best_price, msg)
                                .await
                        {
                            tracing::error!("Failed to handle event: {e:#}");
                        }
                    }

                    round = 1;

                    // abort handler on sending messages over a lost websocket connection.
                    handle.abort();
                }
                Err(e) => {
                    tracing::error!("Could not start up orderbook client: {e:#}");
                }
            };

            if let Err(e) = orderbook_status.send(ServiceStatus::Offline) {
                tracing::warn!("Cannot update orderbook status: {e:#}");
            };

            let retry_interval = WS_RECONNECT_TIMEOUT.mul_f32(round as f32);
            tracing::debug!(
                ?retry_interval,
                "Reconnecting to orderbook WS after timeout"
            );
            tokio::time::sleep(retry_interval).await;
            round *= 2;
        }
    });

    Ok(())
}

async fn handle_orderbook_message(
    orders: Arc<Mutex<Vec<Order>>>,
    cached_best_price: &mut Prices,
    msg: String,
) -> Result<()> {
    let msg =
        serde_json::from_str::<Message>(&msg).context("Could not deserialize orderbook message")?;

    tracing::trace!(%msg, "New orderbook message");

    match msg {
        Message::Authenticated(lsp_config) => {
            tracing::info!("Successfully logged in to 10101 websocket api!");
            state::set_lsp_config(lsp_config.clone());
            event::publish(&EventInternal::Authenticated(lsp_config));
        }
        Message::Rollover(contract_id) => {
            tracing::info!("Received a rollover request from orderbook.");
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::Rollover(TaskStatus::Pending),
            ));

            if let Err(e) = position::handler::rollover(contract_id).await {
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::Rollover(TaskStatus::Failed),
                ));

                bail!("Failed to rollover DLC: {e:#}");
            }
        }
        Message::AsyncMatch { order, filled_with } => {
            let order_id = order.id;
            let order_reason = order.clone().order_reason.into();

            tracing::info!(
                %order_id,
                "Received an async match from orderbook. Reason: {order_reason:?}"
            );

            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::AsyncTrade(order_reason),
            ));

            position::handler::async_trade(order.clone(), filled_with)
                .await
                .with_context(|| format!("Failed to process async trade for order {}", order_id))?;
        }
        Message::Match(filled) => {
            let order_id = filled.order_id;

            tracing::info!(%order_id, "Received match from orderbook");

            position::handler::trade(filled.clone())
                .await
                .with_context(|| {
                    format!("Trade request sent to coordinator for order {order_id} failed")
                })?;
        }
        Message::AllOrders(initial_orders) => {
            let mut orders = orders.lock();
            if !orders.is_empty() {
                tracing::debug!(
                    "Received new set of initial orders from orderbook, \
                     replacing the previously stored orders"
                );
            } else {
                tracing::debug!(?orders, "Received all orders from orderbook");
            }

            *orders = initial_orders;

            // if we receive a full set of new orders, we can clear the cached best price as it is
            // outdated information.
            cached_best_price.clear();
            update_prices_if_needed(cached_best_price, &orders);
        }
        Message::NewOrder(order) => {
            let mut orders = orders.lock();
            orders.push(order);

            update_prices_if_needed(cached_best_price, &orders);
        }
        Message::DeleteOrder(order_id) => {
            let mut orders = orders.lock();

            let found = remove_order(&mut orders, order_id);
            if !found {
                tracing::warn!(%order_id, "Could not remove non-existing order");
            }

            update_prices_if_needed(cached_best_price, &orders);
        }
        Message::Update(updated_order) => {
            let mut orders = orders.lock();

            let found = remove_order(&mut orders, updated_order.id);
            if !found {
                tracing::warn!(?updated_order, "Update without prior knowledge of order");
            }

            orders.push(updated_order);

            update_prices_if_needed(cached_best_price, &orders);
        }
        Message::DlcChannelCollaborativeRevert {
            channel_id,
            coordinator_address,
            coordinator_amount,
            trader_amount,
            execution_price,
        } => {
            tracing::debug!(
                channel_id = %hex::encode(channel_id),
                "Received request to revert channel"
            );

            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::CollabRevert(TaskStatus::Pending),
            ));

            if let Err(err) = ln_dlc::collaborative_revert_channel(
                channel_id,
                coordinator_address,
                coordinator_amount,
                trader_amount,
                execution_price,
            ) {
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::CollabRevert(TaskStatus::Failed),
                ));
                tracing::error!("Could not collaboratively revert channel: {err:#}");
            } else {
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::CollabRevert(TaskStatus::Success),
                ));
            }
        }
        msg @ Message::LimitOrderFilledMatches { .. } | msg @ Message::InvalidAuthentication(_) => {
            tracing::debug!(?msg, "Skipping message from orderbook");
        }
        Message::TradeError { order_id, error } => {
            order::handler::order_failed(
                Some(order_id),
                FailureReason::TradeResponse(error.clone()),
                anyhow!("Coordinator failed to execute trade: {error}"),
            )
            .context("Could not set order to failed")?;
        }
    };

    Ok(())
}

fn update_prices_if_needed(cached_best_price: &mut Prices, orders: &[Order]) {
    let best_price = best_current_price(orders);
    if *cached_best_price != best_price {
        if let Err(e) = position::handler::price_update(best_price.clone()) {
            tracing::error!("Price update from the orderbook failed. Error: {e:#}");
        }
        *cached_best_price = best_price;
    }
}

// Returns true if the order was found and removed
fn remove_order(orders: &mut Vec<Order>, order_id: Uuid) -> bool {
    let mut found = false;
    for (index, element) in orders.iter().enumerate() {
        if element.id == order_id {
            found = true;
            orders.remove(index);
            break;
        }
    }
    found
}
