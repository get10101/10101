use crate::event;
use crate::event::EventInternal;
use crate::event::EventType;

#[derive(Clone)]
pub struct Subscriber {}

/// Subscribes to event relevant for flutter and forwards them to the stream sink.
impl event::subscriber::Subscriber for Subscriber {
    fn notify(&self, event: &EventInternal) {
        match event {
            EventInternal::OrderFilledWith(trade_params) => {
                tokio::spawn({
                    let _trade_params = trade_params.clone();
                    async move {
                        // TODO: Trigger this once we have an orderbook and remove triggering trade
                        // upon order submission
                        // handler::trade(trade_params.clone()).await.unwrap();
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
