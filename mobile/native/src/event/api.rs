use crate::api::WalletInfo;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::trade::order::api::Order;
use crate::trade::position::api::Position;
use core::convert::From;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::StreamSink;

#[frb]
#[derive(Clone)]
pub enum Event {
    Init(String),
    Log(String),
    OrderUpdateNotification(Order),
    WalletInfoUpdateNotification(WalletInfo),
    PositionUpdateNotification(Position),
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
        }
    }
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

    fn filter(&self, event: &EventInternal) -> bool {
        matches!(
            event,
            EventInternal::Init(_)
                | EventInternal::WalletInfoUpdateNotification(_)
                | EventInternal::OrderUpdateNotification(_)
                | EventInternal::PositionUpdateNotification(_)
        )
    }
}

impl FlutterSubscriber {
    pub fn new(stream: StreamSink<Event>) -> Self {
        FlutterSubscriber { stream }
    }
}
