use crate::config;
use crate::health::ServiceStatus;
use crate::trade::position;
use anyhow::Result;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::secp256k1::SECP256K1;
use futures::TryStreamExt;
use orderbook_commons::best_current_price;
use orderbook_commons::Order;
use orderbook_commons::OrderbookMsg;
use orderbook_commons::Signature;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio::task::spawn_blocking;
use uuid::Uuid;

const WS_RECONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const EXPIRED_ORDER_PRUNING_INTERVAL: Duration = Duration::from_secs(30);

pub fn subscribe(
    secret_key: SecretKey,
    runtime: &Runtime,
    orderbook_status: watch::Sender<ServiceStatus>,
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

        loop {
            let url = url.clone();
            let authenticate = authenticate;
            let mut stream =
                spawn_blocking(move || orderbook_client::subscribe_with_authentication(url, authenticate)).await.expect("joined task not to panic");

            if let Err(e) =
                orderbook_status.send(ServiceStatus::Online) {
                    tracing::warn!("Cannot update orderbook status: {e:#}");
                };

            loop {
                match stream.try_next().await {
                    Ok(Some(msg)) => {
                        tracing::debug!(%msg, "New message from orderbook");

                        let msg = match serde_json::from_str::<OrderbookMsg>(&msg) {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::error!(
                                    "Could not deserialize message from orderbook. Error: {e:#}"
                                );
                                continue;
                            }
                        };

                        match msg {
                            OrderbookMsg::Match(filled) => {
                                tracing::info!(order_id = %filled.order_id, "Received match from orderbook");

                                if let Err(e) = position::handler::trade(filled).await {
                                    tracing::error!("Trade request sent to coordinator failed. Error: {e:#}");
                                }
                            },
                            OrderbookMsg::AllOrders(initial_orders) => {
                                let mut orders = orders.lock();
                                if !orders.is_empty() {
                                    tracing::debug!("Received new set of initial orders from orderbook, replacing the previously stored orders");
                                }
                                else {
                                    tracing::debug!(?orders, "Received all orders from orderbook");
                                }
                                *orders = initial_orders;
                                if let Err(e) = position::handler::price_update(best_current_price(&orders)) {
                                    tracing::error!("Price update from the orderbook failed. Error: {e:#}");
                                }
                            },
                            OrderbookMsg::NewOrder(order) => {
                                let mut orders = orders.lock();
                                orders.push(order);
                                if let Err(e) = position::handler::price_update(best_current_price(&orders)) {
                                    tracing::error!("Price update from the orderbook failed. Error: {e:#}");
                                }
                            }
                            OrderbookMsg::DeleteOrder(order_id) => {
                                let mut orders = orders.lock();
                                let found = remove_order(&mut orders, order_id);
                                if !found {
                                    tracing::warn!(%order_id, "Could not remove non-existing order");
                                }
                                if let Err(e) = position::handler::price_update(best_current_price(&orders)) {
                                    tracing::error!("Price update from the orderbook failed. Error: {e:#}");
                                }
                            },
                            OrderbookMsg::Update(updated_order) => {
                                let mut orders = orders.lock();
                                let found = remove_order(&mut orders, updated_order.id);
                                if !found {
                                    tracing::warn!(?updated_order, "Update without prior knowledge of order");
                                }
                                orders.push(updated_order);
                                if let Err(e) = position::handler::price_update(best_current_price(&orders)) {
                                    tracing::error!("Price update from the orderbook failed. Error: {e:#}");
                                }
                            },
                            _ => tracing::debug!(?msg, "Skipping message from orderbook"),
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
