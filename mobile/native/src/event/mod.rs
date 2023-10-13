use crate::api::WalletInfo;
use crate::event::event_hub::get;
use crate::event::subscriber::Subscriber;
use crate::health::ServiceUpdate;
use crate::ln_dlc::ChannelStatus;
use crate::trade::order::Order;
use crate::trade::order::OrderReason;
use crate::trade::position::Position;
use coordinator_commons::TradeParams;
use ln_dlc_node::node::rust_dlc_manager::ChannelId;
use orderbook_commons::Prices;
use std::fmt;
use std::hash::Hash;
use trade::ContractSymbol;

mod event_hub;

pub mod api;
pub mod subscriber;

pub fn subscribe(subscriber: impl Subscriber + 'static + Send + Sync + Clone) {
    get().subscribe(subscriber);
}

pub fn publish(event: &EventInternal) {
    get().publish(event);
}

#[derive(Clone, Debug)]
pub enum EventInternal {
    Init(String),
    Log(String),
    OrderUpdateNotification(Order),
    WalletInfoUpdateNotification(WalletInfo),
    OrderFilledWith(Box<TradeParams>),
    PositionUpdateNotification(Position),
    PositionCloseNotification(ContractSymbol),
    PriceUpdateNotification(Prices),
    ChannelReady(ChannelId),
    PaymentClaimed(u64),
    PaymentSent,
    PaymentFailed,
    ServiceHealthUpdate(ServiceUpdate),
    ChannelStatusUpdate(ChannelStatus),
    BackgroundNotification(BackgroundTask),
}

#[derive(Clone, Debug)]
pub enum BackgroundTask {
    AsyncTrade(OrderReason),
    Rollover(TaskStatus),
    RecoverDlc(TaskStatus),
}

#[derive(Clone, Debug)]
pub enum TaskStatus {
    Pending,
    Failed,
    Success,
}

impl fmt::Display for EventInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventInternal::Init(_) => "Init",
            EventInternal::Log(_) => "Log",
            EventInternal::OrderUpdateNotification(_) => "OrderUpdateNotification",
            EventInternal::WalletInfoUpdateNotification(_) => "WalletInfoUpdateNotification",
            EventInternal::OrderFilledWith(_) => "OrderFilledWith",
            EventInternal::PositionUpdateNotification(_) => "PositionUpdateNotification",
            EventInternal::PositionCloseNotification(_) => "PositionCloseNotification",
            EventInternal::PriceUpdateNotification(_) => "PriceUpdateNotification",
            EventInternal::ChannelReady(_) => "ChannelReady",
            EventInternal::PaymentClaimed(_) => "PaymentClaimed",
            EventInternal::PaymentSent => "PaymentSent",
            EventInternal::PaymentFailed => "PaymentFailed",
            EventInternal::ServiceHealthUpdate(_) => "ServiceHealthUpdate",
            EventInternal::ChannelStatusUpdate(_) => "ChannelStatusUpdate",
            EventInternal::BackgroundNotification(_) => "BackgroundNotification",
        }
        .fmt(f)
    }
}

impl From<EventInternal> for EventType {
    fn from(value: EventInternal) -> Self {
        match value {
            EventInternal::Init(_) => EventType::Init,
            EventInternal::Log(_) => EventType::Log,
            EventInternal::OrderUpdateNotification(_) => EventType::OrderUpdateNotification,
            EventInternal::WalletInfoUpdateNotification(_) => {
                EventType::WalletInfoUpdateNotification
            }
            EventInternal::OrderFilledWith(_) => EventType::OrderFilledWith,
            EventInternal::PositionUpdateNotification(_) => EventType::PositionUpdateNotification,
            EventInternal::PositionCloseNotification(_) => EventType::PositionClosedNotification,
            EventInternal::PriceUpdateNotification(_) => EventType::PriceUpdateNotification,
            EventInternal::ChannelReady(_) => EventType::ChannelReady,
            EventInternal::PaymentClaimed(_) => EventType::PaymentClaimed,
            EventInternal::PaymentSent => EventType::PaymentSent,
            EventInternal::PaymentFailed => EventType::PaymentFailed,
            EventInternal::ServiceHealthUpdate(_) => EventType::ServiceHealthUpdate,
            EventInternal::ChannelStatusUpdate(_) => EventType::ChannelStatusUpdate,
            EventInternal::BackgroundNotification(_) => EventType::BackgroundNotification,
        }
    }
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub enum EventType {
    Init,
    Log,
    OrderUpdateNotification,
    WalletInfoUpdateNotification,
    OrderFilledWith,
    PositionUpdateNotification,
    PositionClosedNotification,
    PriceUpdateNotification,
    ChannelReady,
    PaymentClaimed,
    PaymentSent,
    PaymentFailed,
    ServiceHealthUpdate,
    ChannelStatusUpdate,
    BackgroundNotification,
}
