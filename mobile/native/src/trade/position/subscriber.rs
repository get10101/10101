use crate::event;
use crate::event::EventInternal;
use crate::event::EventType;

#[derive(Clone)]
pub struct Subscriber {}

/// Subscribes to event relevant for flutter and forwards them to the stream sink.
impl event::subscriber::Subscriber for Subscriber {
    fn notify(&self, event: &EventInternal) {
        match event {
            EventInternal::OrderFilledWith(_trade_params) => {
                // TODO: spawn task to handle the trade
            }
            _ => unreachable!("Received Unexpected Event"),
        }
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::OrderFilledWith]
    }
}
