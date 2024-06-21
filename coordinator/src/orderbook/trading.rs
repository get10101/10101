use crate::message::TraderMessage;
use crate::notifications::Notification;
use crate::orderbook::match_order::MatchedOrder;
use crate::orderbook::Orderbook;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;
use xxi_node::commons;
use xxi_node::commons::Message;
use xxi_node::commons::NewOrder;
use xxi_node::commons::Order;
use xxi_node::commons::OrderReason;

/// This value is arbitrarily set to 100 and defines the number of new order messages buffered in
/// the channel.
const ORDERBOOK_BUFFER_SIZE: usize = 100;

#[derive(Debug)]
pub enum OrderbookMessage {
    NewOrder {
        new_order: NewOrder,
        order_reason: OrderReason,
    },
    DeleteOrder(Uuid),
    Update(Order),
}

pub fn spawn_orderbook(
    pool: Pool<ConnectionManager<PgConnection>>,
    notifier: mpsc::Sender<Notification>,
    match_executor: mpsc::Sender<MatchedOrder>,
    tx_orderbook_feed: broadcast::Sender<Message>,
    trader_sender: mpsc::Sender<TraderMessage>,
) -> mpsc::Sender<OrderbookMessage> {
    let (sender, mut receiver) = mpsc::channel::<OrderbookMessage>(ORDERBOOK_BUFFER_SIZE);

    tokio::spawn({
        let notifier = notifier.clone();
        let trader_sender = trader_sender.clone();
        let tx_orderbook_feed = tx_orderbook_feed.clone();

        async move {
            let mut orderbook = match Orderbook::new(
                pool.clone(),
                notifier,
                match_executor,
                trader_sender.clone(),
            )
            .await
            {
                Ok(orderbook) => orderbook,
                Err(e) => {
                    tracing::error!("Failed to initialize orderbook. Error: {e:#}");
                    return;
                }
            };

            while let Some(message) = receiver.recv().await {
                let msg = match message {
                    OrderbookMessage::NewOrder {
                        new_order: NewOrder::Market(new_order),
                        order_reason,
                    } => {
                        orderbook.match_market_order(new_order, order_reason);
                        // TODO(holzeis): Send orderbook updates about updated or removed limit
                        // orders due to matching.
                        None
                    }
                    OrderbookMessage::NewOrder {
                        new_order: NewOrder::Limit(new_order),
                        ..
                    } => {
                        let message = orderbook.add_limit_order(new_order);
                        Some(message)
                    }
                    OrderbookMessage::DeleteOrder(order_id) => orderbook.remove_order(order_id),
                    OrderbookMessage::Update(
                        order @ Order {
                            order_type: commons::OrderType::Limit,
                            ..
                        },
                    ) => {
                        let message = orderbook.update_limit_order(order);
                        Some(message)
                    }
                    OrderbookMessage::Update(Order {
                        order_type: commons::OrderType::Market,
                        ..
                    }) => {
                        tracing::debug!("Ignoring market order update.");
                        None
                    }
                };

                if let Some(msg) = msg {
                    if let Err(e) = tx_orderbook_feed.send(msg) {
                        tracing::error!("Failed to send message. Error: {e:#}");
                    }
                }
            }

            tracing::warn!("Orderbook channel has been closed.");
        }
    });

    sender
}
