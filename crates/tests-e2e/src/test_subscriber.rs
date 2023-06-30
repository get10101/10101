use std::sync::Arc;
use std::sync::Mutex;

use coordinator_commons::TradeParams;
use native::api::WalletInfo;
use native::event::subscriber::Subscriber;
use native::event::EventType;
use native::trade::order::Order;
use native::trade::position::Position;
use tokio::sync::watch;

pub struct Senders {
    wallet_info: watch::Sender<Option<WalletInfo>>,
    order: watch::Sender<Option<Order>>,
    order_filled: watch::Sender<Option<Box<TradeParams>>>,
    position: watch::Sender<Option<Position>>,
    /// Init messages are simple strings
    init_msg: watch::Sender<Option<String>>,
}

/// Subscribes to events destined for the frontend (typically Flutter app) and
/// provides a convenient way to access the current state.
pub struct TestSubscriber {
    wallet_info: watch::Receiver<Option<WalletInfo>>,
    order: watch::Receiver<Option<Order>>,
    order_filled: watch::Receiver<Option<Box<TradeParams>>>,
    position: watch::Receiver<Option<Position>>,
    init_msg: watch::Receiver<Option<String>>,
    // TODO add prices and close position watches
}

impl TestSubscriber {
    pub fn new() -> (Self, ThreadSafeSenders) {
        let (wallet_info_tx, wallet_info_rx) = watch::channel(None);
        let (order_tx, order_rx) = watch::channel(None);
        let (order_filled_tx, order_filled_rx) = watch::channel(None);
        let (position_tx, position_rx) = watch::channel(None);
        let (init_msg_tx, init_msg_rx) = watch::channel(None);

        let senders = Senders {
            wallet_info: wallet_info_tx,
            order: order_tx,
            order_filled: order_filled_tx,
            position: position_tx,
            init_msg: init_msg_tx,
        };

        let rx = Self {
            wallet_info: wallet_info_rx,
            order_filled: order_filled_rx,
            order: order_rx,
            position: position_rx,
            init_msg: init_msg_rx,
        };
        (rx, ThreadSafeSenders(Arc::new(Mutex::new(senders))))
    }

    pub fn wallet_info(&self) -> Option<WalletInfo> {
        self.wallet_info.borrow().as_ref().cloned()
    }

    pub fn order(&self) -> Option<Order> {
        self.order.borrow().as_ref().copied()
    }

    pub fn order_filled(&self) -> Option<Box<TradeParams>> {
        self.order_filled.borrow().as_ref().cloned()
    }

    pub fn position(&self) -> Option<Position> {
        self.position.borrow().as_ref().cloned()
    }

    pub fn init_msg(&self) -> Option<String> {
        self.init_msg.borrow().as_ref().cloned()
    }
}

impl Subscriber for Senders {
    fn notify(&self, event: &native::event::EventInternal) {
        match event {
            native::event::EventInternal::Init(init) => {
                tracing::info!(%init, "Received init message");
                self.init_msg
                    .send(Some(init.to_string()))
                    .expect("to be able to send update");
            }
            native::event::EventInternal::Log(_log) => {
                // Ignore log events for now
            }
            native::event::EventInternal::OrderUpdateNotification(order) => {
                tracing::trace!(?order, "Received order update event");
                self.order
                    .send(Some(*order))
                    .expect("to be able to send update");
            }
            native::event::EventInternal::WalletInfoUpdateNotification(wallet_info) => {
                tracing::trace!(?wallet_info, "Received wallet info update event");
                self.wallet_info
                    .send(Some(wallet_info.clone()))
                    .expect("to be able to send update");
            }
            native::event::EventInternal::OrderFilledWith(order_filled) => {
                tracing::trace!(?order_filled, "Received order filled event");
                self.order_filled
                    .send(Some(order_filled.clone()))
                    .expect("to be able to send update");
            }
            native::event::EventInternal::PositionUpdateNotification(position) => {
                tracing::trace!(?position, "Received position update event");
                self.position
                    .send(Some(position.clone()))
                    .expect("to be able to send update");
            }
            native::event::EventInternal::PositionCloseNotification(contract_symbol) => {
                tracing::trace!(?contract_symbol, "Received position close event");
            }
            native::event::EventInternal::PriceUpdateNotification(prices) => {
                tracing::trace!(?prices, "Received price update event");
                // TODO: Add prices from orderbook_commons
            }
            native::event::EventInternal::ChannelReady(channel_id) => {
                tracing::trace!(?channel_id, "Received channel ready event");
            }
            native::event::EventInternal::PaymentClaimed(amount_msats) => {
                tracing::trace!(amount_msats, "Received payment claimed event");
            }
        }
    }

    fn events(&self) -> Vec<EventType> {
        vec![
            EventType::Init,
            EventType::WalletInfoUpdateNotification,
            EventType::OrderUpdateNotification,
            EventType::PositionUpdateNotification,
            EventType::PositionClosedNotification,
            EventType::PriceUpdateNotification,
        ]
    }
}

// This is so cumbersome because of EventHub requiring a Send + Sync + Clone subscriber
#[derive(Clone)]
pub struct ThreadSafeSenders(Arc<Mutex<Senders>>);

impl Subscriber for ThreadSafeSenders {
    fn notify(&self, event: &native::event::EventInternal) {
        let guard = self.0.lock().expect("mutex not poisoned");
        guard.notify(event);
    }

    fn events(&self) -> Vec<EventType> {
        let guard = self.0.lock().expect("mutex not poisoned");
        guard.events()
    }
}
