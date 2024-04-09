use native::api::WalletInfo;
use native::event::subscriber::Subscriber;
use native::event::EventInternal;
use native::event::EventType;
use parking_lot::Mutex;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::watch;

pub struct Senders {
    wallet_info: watch::Sender<Option<WalletInfo>>,
    ask_price_info: watch::Sender<Option<Decimal>>,
    bid_price_info: watch::Sender<Option<Decimal>>,
}

/// Subscribes to events destined for the frontend (typically Flutter app) and
/// provides a convenient way to access the current state.
pub struct AppSubscribers {
    wallet_info: watch::Receiver<Option<WalletInfo>>,
    ask_price_info: watch::Receiver<Option<Decimal>>,
    bid_price_info: watch::Receiver<Option<Decimal>>,
}

impl AppSubscribers {
    pub async fn new() -> (Self, ThreadSafeSenders) {
        let (wallet_info_tx, wallet_info_rx) = watch::channel(None);
        let (ask_price_info_tx, ask_price_info_rx) = watch::channel(None);
        let (bid_price_info_tx, bid_price_info_rx) = watch::channel(None);

        let senders = Senders {
            wallet_info: wallet_info_tx,
            ask_price_info: ask_price_info_tx,
            bid_price_info: bid_price_info_tx,
        };

        let subscriber = Self {
            wallet_info: wallet_info_rx,
            ask_price_info: ask_price_info_rx,
            bid_price_info: bid_price_info_rx,
        };
        (subscriber, ThreadSafeSenders(Arc::new(Mutex::new(senders))))
    }

    pub fn wallet_info(&self) -> Option<WalletInfo> {
        self.wallet_info.borrow().as_ref().cloned()
    }
    pub fn ask_price(&self) -> Option<Decimal> {
        self.ask_price_info.borrow().as_ref().cloned()
    }

    pub fn bid_price(&self) -> Option<Decimal> {
        self.bid_price_info.borrow().as_ref().cloned()
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
            EventType::AskPriceUpdateNotification,
            EventType::BidPriceUpdateNotification,
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
        if let EventInternal::AskPriceUpdateNotification(price) = event {
            self.ask_price_info.send(Some(*price))?;
        }
        if let EventInternal::BidPriceUpdateNotification(price) = event {
            self.bid_price_info.send(Some(*price))?;
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
