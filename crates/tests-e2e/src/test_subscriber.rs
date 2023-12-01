use commons::Prices;
use commons::TradeParams;
use native::api::ContractSymbol;
use native::api::WalletInfo;
use native::event::subscriber::Subscriber;
use native::event::EventType;
use native::health::Service;
use native::health::ServiceStatus;
use native::health::ServiceUpdate;
use native::ln_dlc::ChannelStatus;
use native::trade::order::Order;
use native::trade::position::Position;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;

pub struct Senders {
    wallet_info: watch::Sender<Option<WalletInfo>>,
    order: watch::Sender<Option<Order>>,
    order_filled: watch::Sender<Option<Box<TradeParams>>>,
    position: watch::Sender<Option<Position>>,
    /// Init messages are simple strings
    init_msg: watch::Sender<Option<String>>,
    prices: watch::Sender<Option<Prices>>,
    position_close: watch::Sender<Option<ContractSymbol>>,
    service: watch::Sender<Option<ServiceUpdate>>,
    channel_status: watch::Sender<Option<ChannelStatus>>,
}

/// Subscribes to events destined for the frontend (typically Flutter app) and
/// provides a convenient way to access the current state.
pub struct TestSubscriber {
    wallet_info: watch::Receiver<Option<WalletInfo>>,
    order: watch::Receiver<Option<Order>>,
    order_filled: watch::Receiver<Option<Box<TradeParams>>>,
    position: watch::Receiver<Option<Position>>,
    init_msg: watch::Receiver<Option<String>>,
    prices: watch::Receiver<Option<Prices>>,
    position_close: watch::Receiver<Option<ContractSymbol>>,
    services: Arc<Mutex<HashMap<Service, ServiceStatus>>>,
    channel_status: watch::Receiver<Option<ChannelStatus>>,
    _service_map_updater: tokio::task::JoinHandle<()>,
}

impl TestSubscriber {
    pub async fn new() -> (Self, ThreadSafeSenders) {
        let (wallet_info_tx, wallet_info_rx) = watch::channel(None);
        let (order_tx, order_rx) = watch::channel(None);
        let (order_filled_tx, order_filled_rx) = watch::channel(None);
        let (position_tx, position_rx) = watch::channel(None);
        let (init_msg_tx, init_msg_rx) = watch::channel(None);
        let (prices_tx, prices_rx) = watch::channel(None);
        let (position_close_tx, position_close_rx) = watch::channel(None);
        let (service_tx, mut service_rx) = watch::channel(None);
        let (channel_status_tx, channel_status_rx) = watch::channel(None);

        let senders = Senders {
            wallet_info: wallet_info_tx,
            order: order_tx,
            order_filled: order_filled_tx,
            position: position_tx,
            init_msg: init_msg_tx,
            prices: prices_tx,
            position_close: position_close_tx,
            service: service_tx,
            channel_status: channel_status_tx,
        };

        let services = Arc::new(Mutex::new(HashMap::new()));

        let _service_map_updater = {
            let services = services.clone();
            tokio::spawn(async move {
                while let Ok(()) = service_rx.changed().await {
                    if let Some(ServiceUpdate { service, status }) = *service_rx.borrow() {
                        tracing::debug!(?service, ?status, "Updating status in the services map");
                        services.lock().insert(service, status);
                    }
                }
                panic!("service_rx channel closed");
            })
        };

        let subscriber = Self {
            wallet_info: wallet_info_rx,
            order_filled: order_filled_rx,
            order: order_rx,
            position: position_rx,
            init_msg: init_msg_rx,
            prices: prices_rx,
            position_close: position_close_rx,
            services,
            channel_status: channel_status_rx,
            _service_map_updater,
        };
        (subscriber, ThreadSafeSenders(Arc::new(Mutex::new(senders))))
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

    pub fn prices(&self) -> Option<Prices> {
        self.prices.borrow().as_ref().cloned()
    }

    pub fn position_close(&self) -> Option<ContractSymbol> {
        self.position_close.borrow().as_ref().cloned()
    }

    pub fn status(&self, service: Service) -> ServiceStatus {
        self.services
            .lock()
            .get(&service)
            .copied()
            .unwrap_or_default()
    }

    pub fn channel_status(&self) -> Option<ChannelStatus> {
        self.channel_status.borrow().as_ref().cloned()
    }
}

impl Subscriber for Senders {
    fn notify(&self, event: &native::event::EventInternal) {
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
    fn handle_event(&self, event: &native::event::EventInternal) -> anyhow::Result<()> {
        tracing::trace!(?event, "Received event");
        match event {
            native::event::EventInternal::Init(init) => {
                self.init_msg.send(Some(init.to_string()))?;
            }
            native::event::EventInternal::Log(_log) => {
                // Ignore log events for now
            }
            native::event::EventInternal::OrderUpdateNotification(order) => {
                self.order.send(Some(*order))?;
            }
            native::event::EventInternal::WalletInfoUpdateNotification(wallet_info) => {
                self.wallet_info.send(Some(wallet_info.clone()))?;
            }
            native::event::EventInternal::OrderFilledWith(order_filled) => {
                self.order_filled.send(Some(order_filled.clone()))?;
            }
            native::event::EventInternal::PositionUpdateNotification(position) => {
                self.position.send(Some(position.clone()))?;
            }
            native::event::EventInternal::PositionCloseNotification(contract_symbol) => {
                self.position_close.send(Some(*contract_symbol))?;
            }
            native::event::EventInternal::PriceUpdateNotification(prices) => {
                self.prices.send(Some(prices.clone()))?;
            }
            native::event::EventInternal::ServiceHealthUpdate(update) => {
                self.service.send(Some(update.clone()))?;
            }
            native::event::EventInternal::ChannelStatusUpdate(update) => {
                self.channel_status.send(Some(*update))?;
            }
            native::event::EventInternal::ChannelReady(_channel_id) => {
                unreachable!("ChannelReady event should not be sent to the subscriber");
            }
            native::event::EventInternal::PaymentClaimed(_amount_msats) => {
                unreachable!("PaymentClaimed event should not be sent to the subscriber");
            }
            native::event::EventInternal::BackgroundNotification(_task) => {
                // ignored
            }
            native::event::EventInternal::PaymentSent => {
                unreachable!("PaymentSent event should not be sent to the subscriber");
            }
            native::event::EventInternal::PaymentFailed => {
                unreachable!("PaymentFailed event should not be sent to the subscriber");
            }
            native::event::EventInternal::SpendableOutputs => {
                unreachable!("SpendableOutputs event should not be sent to the subscriber");
            }
            native::event::EventInternal::Authenticated(_) => {
                // ignored
            }
        }
        Ok(())
    }
}

// This is so cumbersome because of EventHub requiring a Send + Sync + Clone subscriber
#[derive(Clone)]
pub struct ThreadSafeSenders(Arc<Mutex<Senders>>);

impl Subscriber for ThreadSafeSenders {
    fn notify(&self, event: &native::event::EventInternal) {
        self.0.lock().notify(event)
    }

    fn events(&self) -> Vec<EventType> {
        self.0.lock().events()
    }
}
