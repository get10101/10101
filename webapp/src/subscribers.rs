use commons::Prices;
use native::api::WalletInfo;
use native::event::subscriber::Subscriber;
use native::event::EventInternal;
use native::event::EventType;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::watch;

pub struct Senders {
    wallet_info: watch::Sender<Option<WalletInfo>>,
    price_info: watch::Sender<Option<Prices>>,
}

/// Subscribes to events destined for the frontend (typically Flutter app) and
/// provides a convenient way to access the current state.
pub struct AppSubscribers {
    wallet_info: watch::Receiver<Option<WalletInfo>>,
    price_info: watch::Receiver<Option<Prices>>,
}

impl AppSubscribers {
    pub async fn new() -> (Self, ThreadSafeSenders) {
        let (wallet_info_tx, wallet_info_rx) = watch::channel(None);
        let (price_info_tx, price_info_rx) = watch::channel(None);

        let senders = Senders {
            wallet_info: wallet_info_tx,
            price_info: price_info_tx,
        };

        let subscriber = Self {
            wallet_info: wallet_info_rx,
            price_info: price_info_rx,
        };
        (subscriber, ThreadSafeSenders(Arc::new(Mutex::new(senders))))
    }

    pub fn wallet_info(&self) -> Option<WalletInfo> {
        self.wallet_info.borrow().as_ref().cloned()
    }
    pub fn orderbook_info(&self) -> Option<Prices> {
        self.price_info.borrow().as_ref().cloned()
    }
}

impl Subscriber for Senders {
    fn notify(&self, event: &EventInternal) {
        if let Err(e) = self.handle_event(event) {
            tracing::error!(?e, ?event, "Failed to handle event");
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
            EventType::ServiceHealthUpdate,
            EventType::ChannelStatusUpdate,
        ]
    }
}

impl Senders {
    fn handle_event(&self, event: &EventInternal) -> anyhow::Result<()> {
        tracing::trace!(?event, "Received event");
        if let EventInternal::WalletInfoUpdateNotification(wallet_info) = event {
            self.wallet_info.send(Some(wallet_info.clone()))?;
        }
        if let EventInternal::PriceUpdateNotification(prices) = event {
            self.price_info.send(Some(prices.clone()))?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ThreadSafeSenders(Arc<Mutex<Senders>>);

impl Subscriber for ThreadSafeSenders {
    fn notify(&self, event: &EventInternal) {
        self.0.lock().notify(event)
    }

    fn events(&self) -> Vec<EventType> {
        self.0.lock().events()
    }
}
