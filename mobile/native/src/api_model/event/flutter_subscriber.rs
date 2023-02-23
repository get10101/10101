use crate::api_model::event::Event;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use flutter_rust_bridge::StreamSink;

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
