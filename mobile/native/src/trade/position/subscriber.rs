use crate::event;
use crate::event::{EventInternal, EventType};
use crate::trade::position::handler;

#[derive(Clone)]
pub struct Subscriber {}

/// Subscribes to event relevant for flutter and forwards them to the stream sink.
impl event::subscriber::Subscriber for Subscriber {
    fn notify(&self, event: &EventInternal) {
        match event {
            EventInternal::OrderFilledWith(trade_params) => {
                tokio::spawn({
                    let trade_params = trade_params.clone();
                    async move {
                        handler::trade(trade_params.clone()).await;
                    }
                });
            }
            _ => unreachable!("Received Unexpected Event"),
        }
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::OrderFilledWith]
    }
}
