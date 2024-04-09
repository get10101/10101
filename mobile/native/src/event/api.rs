use crate::api::DlcChannel;
use crate::api::TenTenOneConfig;
use crate::api::WalletInfo;
use crate::dlc_channel;
use crate::event;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::health::ServiceUpdate;
use crate::trade::order::api::Order;
use crate::trade::order::api::OrderReason;
use crate::trade::position::api::Position;
use core::convert::From;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;
use rust_decimal::prelude::ToPrimitive;
use trade::ContractSymbol;

#[frb]
#[derive(Clone)]
pub enum Event {
    Init(String),
    Log(String),
    OrderUpdateNotification(Order),
    WalletInfoUpdateNotification(WalletInfo),
    PositionUpdateNotification(Position),
    PositionClosedNotification(PositionClosed),
    AskPriceUpdateNotification(f32),
    BidPriceUpdateNotification(f32),
    ServiceHealthUpdate(ServiceUpdate),
    BackgroundNotification(BackgroundTask),
    Authenticated(TenTenOneConfig),
    DlcChannelEvent(DlcChannel),
}

#[frb]
#[derive(Clone)]
pub enum BackgroundTask {
    /// The order book submitted an trade which was matched asynchronously while the app was
    /// offline.
    AsyncTrade(OrderReason),
    /// The order book submitted its intention to rollover the about to expire position.
    Rollover(TaskStatus),
    /// The app was started with a dlc channel in an intermediate state. This task is in pending
    /// until the dlc protocol reaches a final state.
    RecoverDlc(TaskStatus),
    /// The coordinator wants to collaboratively close a ln channel with a stuck position.
    CollabRevert(TaskStatus),
    /// The app is performing a full sync of the on-chain wallet.
    FullSync(TaskStatus),
}

impl From<EventInternal> for Event {
    fn from(value: EventInternal) -> Self {
        match value {
            EventInternal::Init(value) => Event::Init(value),
            EventInternal::Log(value) => Event::Log(value),
            EventInternal::OrderUpdateNotification(value) => {
                Event::OrderUpdateNotification(value.into())
            }
            EventInternal::WalletInfoUpdateNotification(value) => {
                Event::WalletInfoUpdateNotification(value)
            }
            EventInternal::OrderFilledWith(_) => {
                unreachable!("This internal event is not exposed to the UI")
            }
            EventInternal::PositionUpdateNotification(position) => {
                Event::PositionUpdateNotification(position.into())
            }
            EventInternal::PositionCloseNotification(contract_symbol) => {
                Event::PositionClosedNotification(PositionClosed { contract_symbol })
            }
            EventInternal::ServiceHealthUpdate(update) => Event::ServiceHealthUpdate(update),
            EventInternal::BackgroundNotification(task) => {
                Event::BackgroundNotification(task.into())
            }
            EventInternal::SpendableOutputs => {
                unreachable!("This internal event is not exposed to the UI")
            }
            EventInternal::Authenticated(lsp_config) => Event::Authenticated(lsp_config.into()),
            EventInternal::DlcChannelEvent(channel) => {
                Event::DlcChannelEvent(dlc_channel::DlcChannel::from(channel))
            }
            EventInternal::AskPriceUpdateNotification(price) => {
                Event::AskPriceUpdateNotification(price.to_f32().expect("to fit"))
            }
            EventInternal::BidPriceUpdateNotification(price) => {
                Event::BidPriceUpdateNotification(price.to_f32().expect("to fit"))
            }
        }
    }
}

/// Wrapper struct needed by frb
///
/// The mirrored `ContractSymbol` does not get picked up correctly when using it directly as
/// type in an enum variant, so we wrap it in a struct.
#[frb]
#[derive(Clone, Copy)]
pub struct PositionClosed {
    pub contract_symbol: ContractSymbol,
}

#[derive(Clone)]
pub struct FlutterSubscriber {
    stream: StreamSink<Event>,
}

/// Subscribes to event relevant for flutter and forwards them to the stream sink.
impl Subscriber for FlutterSubscriber {
    fn notify(&self, event: &EventInternal) {
        self.stream.add(event.clone().into());
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
            EventType::BackgroundNotification,
            EventType::PaymentClaimed,
            EventType::PaymentSent,
            EventType::PaymentFailed,
            EventType::Authenticated,
            EventType::DlcChannelEvent,
        ]
    }
}

impl FlutterSubscriber {
    pub fn new(stream: StreamSink<Event>) -> Self {
        FlutterSubscriber { stream }
    }
}

impl From<event::BackgroundTask> for BackgroundTask {
    fn from(value: event::BackgroundTask) -> Self {
        match value {
            event::BackgroundTask::AsyncTrade(order_reason) => {
                BackgroundTask::AsyncTrade(order_reason.into())
            }
            event::BackgroundTask::Rollover(status) => BackgroundTask::Rollover(status.into()),
            event::BackgroundTask::RecoverDlc(status) => BackgroundTask::RecoverDlc(status.into()),
            event::BackgroundTask::CollabRevert(status) => {
                BackgroundTask::CollabRevert(status.into())
            }
            event::BackgroundTask::FullSync(status) => BackgroundTask::FullSync(status.into()),
        }
    }
}

#[frb]
#[derive(Clone)]
pub enum TaskStatus {
    Pending,
    Failed,
    Success,
}

impl From<event::TaskStatus> for TaskStatus {
    fn from(value: event::TaskStatus) -> Self {
        match value {
            event::TaskStatus::Pending => TaskStatus::Pending,
            event::TaskStatus::Failed => TaskStatus::Failed,
            event::TaskStatus::Success => TaskStatus::Success,
        }
    }
}
