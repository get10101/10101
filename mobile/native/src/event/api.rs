use crate::api::WalletInfo;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::trade::order::api::Order;
use crate::trade::position::api::Position;
use core::convert::From;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;
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
        ]
    }
}

impl FlutterSubscriber {
    pub fn new(stream: StreamSink<Event>) -> Self {
        FlutterSubscriber { stream }
    }
}
