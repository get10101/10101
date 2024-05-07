use crate::config;
use crate::dlc;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::health::ServiceStatus;
use crate::state;
use crate::trade::order;
use crate::trade::order::FailureReason;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::SECP256K1;
use futures::SinkExt;
use futures::TryStreamExt;
use parking_lot::Mutex;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;
use tokio_tungstenite_wasm as tungstenite;
use xxi_node::commons::best_ask_price;
use xxi_node::commons::best_bid_price;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::Direction;
use xxi_node::commons::Message;
use xxi_node::commons::Order;
use xxi_node::commons::OrderState;
use xxi_node::commons::OrderbookRequest;
use xxi_node::commons::Signature;

// Set to the same timeout as the p2p connection reconnect
const WS_RECONNECT_TIMEOUT: Duration = Duration::from_secs(1);

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

        let fcm_token = if fcm_token.is_empty() {
            None
        } else {
            Some(fcm_token)
        };

        loop {
            let url = url.clone();
            let fcm_token = fcm_token.clone();
            let version = env!("CARGO_PKG_VERSION").to_string();
            let os = std::env::consts::OS.to_string();
            match orderbook_client::subscribe_with_authentication(url, authenticate, fcm_token, Some(version), Some(os))
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

                    let mut cached_best_price: HashMap<Direction, Decimal> = HashMap::new();
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

            // Retry at least every second. We do this as it the p2p connection is not debouncing,
            // thus it could happen after a restart that the p2p connection is established, but the
            // websocket connection is still waiting to retry. This could have implications when the
            // coordinator returns an error on the websocket which the app is not ready to process.
            //
            // Note, this is the same issue for why we originally moved to 10101 Messages, we should
            // think about a similar way to return protocol errors via the p2p connection.
            tracing::debug!(
                ?WS_RECONNECT_TIMEOUT,
                "Reconnecting to orderbook WS after timeout"
            );
            tokio::time::sleep(WS_RECONNECT_TIMEOUT).await;
        }
    });

    Ok(())
}

async fn handle_orderbook_message(
    orders: Arc<Mutex<Vec<Order>>>,
    cached_best_price: &mut HashMap<Direction, Decimal>,
    msg: String,
) -> Result<()> {
    let msg =
        serde_json::from_str::<Message>(&msg).context("Could not deserialize orderbook message")?;

    tracing::trace!(%msg, "New orderbook message");

    match msg {
        Message::Authenticated(config) => {
            tracing::info!(
                referral_status = ?config.referral_status,
                "Successfully logged in to 10101 websocket api!");
            state::set_tentenone_config(config.clone());
            event::publish(&EventInternal::Authenticated(config));
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
            update_both_prices_if_needed(cached_best_price, &orders);
        }
        Message::NewOrder(order) => {
            let mut orders = orders.lock();
            let direction = order.direction;
            orders.push(order);

            match direction {
                Direction::Long => update_bid_price_if_needed(cached_best_price, orders.as_slice()),
                Direction::Short => {
                    update_ask_price_if_needed(cached_best_price, orders.as_slice())
                }
            }
        }
        Message::DeleteOrder(order_id) => {
            let mut orders = orders.lock();

            let old_length = orders.len();
            orders.retain(|order| order.id != order_id);
            let new_length = orders.len();

            if old_length == new_length {
                tracing::warn!(%order_id, "Could not remove non-existing order");
            }

            update_both_prices_if_needed(cached_best_price, &orders);
        }
        Message::Update(updated_order) => {
            let mut orders = orders.lock();

            let old_length = orders.len();
            orders.retain(|order| order.id != updated_order.id);
            let new_length = orders.len();

            if old_length == new_length {
                tracing::warn!(?updated_order, "Update without prior knowledge of order");
            }

            if updated_order.order_state == OrderState::Open {
                orders.push(updated_order);
            }

            update_both_prices_if_needed(cached_best_price, &orders);
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

            if let Err(err) = dlc::collaborative_revert_channel(
                channel_id,
                coordinator_address,
                coordinator_amount,
                trader_amount,
                execution_price,
            ) {
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::CollabRevert(TaskStatus::Failed(format!("{err:#}"))),
                ));
                tracing::error!("Could not collaboratively revert channel: {err:#}");
            } else {
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::CollabRevert(TaskStatus::Success),
                ));
            }
        }
        msg @ Message::InvalidAuthentication(_) => {
            tracing::debug!(?msg, "Skipping message from orderbook");
        }
        Message::TradeError { order_id, error } => {
            order::handler::order_failed(
                Some(order_id),
                FailureReason::TradeResponse(error.to_string()),
                error.into(),
            )
            .context("Could not set order to failed")?;
        }
        Message::RolloverError { error } => {
            tracing::error!("Failed to rollover position: {error:#}");
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::Rollover(TaskStatus::Failed(format!("{error:#}"))),
            ));
        }
    };

    Ok(())
}

fn update_both_prices_if_needed(
    cached_best_price: &mut HashMap<Direction, Decimal>,
    orders: &[Order],
) {
    update_bid_price_if_needed(cached_best_price, orders);
    update_ask_price_if_needed(cached_best_price, orders);
}

fn update_bid_price_if_needed(
    cached_best_price: &mut HashMap<Direction, Decimal>,
    orders: &[Order],
) {
    let bid_price = best_bid_price(orders, ContractSymbol::BtcUsd);

    update_price_if_needed(cached_best_price, bid_price, Direction::Long);
}

fn update_ask_price_if_needed(
    cached_best_price: &mut HashMap<Direction, Decimal>,
    orders: &[Order],
) {
    let ask_price = best_ask_price(orders, ContractSymbol::BtcUsd);

    update_price_if_needed(cached_best_price, ask_price, Direction::Short);
}

fn update_price_if_needed(
    cached_best_price: &mut HashMap<Direction, Decimal>,
    new_price: Option<Decimal>,
    direction: Direction,
) {
    if let Some(new_price) = new_price {
        if let Some(cached_price) = cached_best_price.get(&direction) {
            if *cached_price != new_price {
                update_price(direction, new_price);
                cached_best_price.insert(direction, new_price);
            }
        } else {
            update_price(direction, new_price);
        }
    }
}

fn update_price(direction: Direction, new_price: Decimal) {
    tracing::trace!(%new_price, direction = %direction, "New price");
    match direction {
        Direction::Long => {
            event::publish(&EventInternal::BidPriceUpdateNotification(new_price));
        }
        Direction::Short => {
            event::publish(&EventInternal::AskPriceUpdateNotification(new_price));
        }
    }
}
