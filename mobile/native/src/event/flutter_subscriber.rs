use crate::event::subscriber::Subscriber;
use crate::event::Event;
use flutter_rust_bridge::StreamSink;

#[derive(Clone)]
pub struct FlutterSubscriber {
    stream: StreamSink<Event>,
}

/// Subscribes to event relevant for flutter and forwards them to the stream sink.
impl Subscriber for FlutterSubscriber {
    fn notify(&self, event: &Event) {
        self.stream.add(event.clone());
    }

    fn filter(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::Init(_) | Event::WalletInfo(_) | Event::OrderUpdateNotification(_)
        )
    }
}

impl FlutterSubscriber {
    pub fn new(stream: StreamSink<Event>) -> Self {
        FlutterSubscriber { stream }
    }
}
