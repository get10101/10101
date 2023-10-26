use crate::config;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::health::ServiceStatus;
use crate::ln_dlc;
use crate::trade::position;
use anyhow::Result;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::secp256k1::SECP256K1;
use futures::TryStreamExt;
use orderbook_commons::best_current_price;
use orderbook_commons::Message;
use orderbook_commons::Order;
use orderbook_commons::Prices;
use orderbook_commons::Signature;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tokio::sync::watch;
use uuid::Uuid;

/// The reconnect timeout should be high enough for the coordinator to get ready after a restart. If
/// we reconnect too early we may not be ready process messages which require DLC actions.
const WS_RECONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const EXPIRED_ORDER_PRUNING_INTERVAL: Duration = Duration::from_secs(30);

pub fn subscribe(
    secret_key: SecretKey,
    runtime: &Runtime,
    orderbook_status: watch::Sender<ServiceStatus>,
    fcm_token: String,
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
                        *orders = orders.iter().filter(|order| order.expiry >= OffsetDateTime::now_utc()).cloned().collect::<Vec<_>>();
                        let orders_after_pruning = orders.len();

                        if orders_after_pruning < orders_before_pruning {
                            let amount_pruned = orders_before_pruning - orders_after_pruning;
                            tracing::debug!(
                                orders_before_pruning,
                                orders_after_pruning,
                                "Pruned {amount_pruned} expired orders"
                            );

                            // Current best price might have changed
                            if let Err(e) = position::handler::price_update(best_current_price(&orders)) {
                                tracing::error!("Price update from the orderbook failed. Error: {e:#}");
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

        loop {
            let url = url.clone();
            let authenticate = authenticate;
            let fcm_token = fcm_token.clone();
            match orderbook_client::subscribe_with_authentication(url, authenticate, fcm_token).await {
                Ok((_, mut stream)) => {
                    if let Err(e) =
                        orderbook_status.send(ServiceStatus::Online) {
                            tracing::warn!("Cannot update orderbook status: {e:#}");
                        };

                    let mut cached_best_price : Prices = HashMap::new();
                    loop {
                        match stream.try_next().await {
                            Ok(Some(msg)) => {
                                let msg = match serde_json::from_str::<Message>(&msg) {
                                    Ok(msg) => {
                                        tracing::debug!(%msg, "New message from orderbook");
                                        msg
                                    },
                                    Err(e) => {
                                        tracing::error!(
                                            "Could not deserialize message from orderbook. Error: {e:#}"
                                        );
                                        continue;
                                    }
                                };

                                match msg {
                                    Message::Rollover(contract_id) => {
                                        tracing::info!("Received a rollover request from orderbook.");
                                        event::publish(&EventInternal::BackgroundNotification(BackgroundTask::Rollover(TaskStatus::Pending)));

                                        if let Err(e) = position::handler::rollover(contract_id).await {
                                            tracing::error!("Failed to rollover dlc. Error: {e:#}");
                                            event::publish(&EventInternal::BackgroundNotification(BackgroundTask::Rollover(TaskStatus::Failed)));
                                        }
                                    },
                                    Message::AsyncMatch { order, filled_with } => {
                                        let order_reason = order.clone().order_reason.into();
                                        tracing::info!(order_id = %order.id, "Received an async match from orderbook. Reason: {order_reason:?}");
                                        event::publish(&EventInternal::BackgroundNotification(BackgroundTask::AsyncTrade(order_reason)));

                                        if let Err(e) = position::handler::async_trade(order.clone(), filled_with).await {
                                            tracing::error!(order_id = %order.id, "Failed to process async trade. Error: {e:#}");
                                        }
                                    },
                                    Message::Match(filled) => {
                                        tracing::info!(order_id = %filled.order_id, "Received match from orderbook");

                                        if let Err(e) = position::handler::trade(filled.clone()).await {
                                            tracing::error!(order_id = %filled.order_id, "Trade request sent to coordinator failed. Error: {e:#}");
                                        }
                                    },
                                    Message::AllOrders(initial_orders) => {
                                        let mut orders = orders.lock();
                                        if !orders.is_empty() {
                                            tracing::debug!("Received new set of initial orders from orderbook, replacing the previously stored orders");
                                        }
                                        else {
                                            tracing::debug!(?orders, "Received all orders from orderbook");
                                        }
                                        *orders = initial_orders;
                                        update_prices_if_needed(&mut cached_best_price, &orders);
                                    },
                                    Message::NewOrder(order) => {
                                        let mut orders = orders.lock();
                                        orders.push(order);
                                        update_prices_if_needed(&mut cached_best_price, &orders);
                                    }
                                    Message::DeleteOrder(order_id) => {
                                        let mut orders = orders.lock();
                                        let found = remove_order(&mut orders, order_id);
                                        if !found {
                                            tracing::warn!(%order_id, "Could not remove non-existing order");
                                        }
                                        update_prices_if_needed(&mut cached_best_price, &orders);
                                    },
                                    Message::Update(updated_order) => {
                                        let mut orders = orders.lock();
                                        let found = remove_order(&mut orders, updated_order.id);
                                        if !found {
                                            tracing::warn!(?updated_order, "Update without prior knowledge of order");
                                        }
                                        orders.push(updated_order);
                                        update_prices_if_needed(&mut cached_best_price, &orders);
                                    },
                                    msg @ Message::LimitOrderFilledMatches { .. } |
                                    msg @ Message::InvalidAuthentication(_) |
                                    msg @ Message::Authenticated => {
                                        tracing::debug!(?msg, "Skipping message from orderbook");
                                    }
                                    Message::CollaborativeRevert { channel_id, coordinator_address, coordinator_amount, trader_amount, execution_price, outpoint } => {
                                        tracing::debug!("Received request to revert channel");
                                        event::publish(&EventInternal::BackgroundNotification(BackgroundTask::CollabRevert(TaskStatus::Pending)));
                                        if let Err(err) = ln_dlc::collaborative_revert_channel(channel_id, coordinator_address, coordinator_amount, trader_amount, execution_price, outpoint) {
                                            event::publish(&EventInternal::BackgroundNotification(BackgroundTask::CollabRevert(TaskStatus::Failed)));
                                            tracing::error!("Could not collaboratively revert channel {err:#}");
                                        } else {
                                            event::publish(&EventInternal::BackgroundNotification(BackgroundTask::CollabRevert(TaskStatus::Success)));
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::warn!("Orderbook WS stream closed");
                                break;
                            }
                            Err(error) => {
                                tracing::warn!(%error, "Orderbook WS stream closed with error");
                                break;
                            }
                        }
                    };
                },
                Err(e) => {
                    tracing::error!("Could not start up orderbook client: {e:#}");
                },
            };

            if let Err(e) =
            orderbook_status.send(ServiceStatus::Offline) {
                tracing::warn!("Cannot update orderbook status: {e:#}");
            };

            tokio::time::sleep(WS_RECONNECT_TIMEOUT).await;
            tracing::debug!(?WS_RECONNECT_TIMEOUT, "Reconnecting to orderbook WS after timeout");
        }
    });

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
