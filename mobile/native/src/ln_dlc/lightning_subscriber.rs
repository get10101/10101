use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::node::Node;
use lightning::events::Event;
use tokio::sync::watch::Receiver;

impl Node {
    pub async fn listen_for_lightning_events(&self, mut event_receiver: Receiver<Option<Event>>) {
        loop {
            match event_receiver.changed().await {
                Ok(()) => {
                    if let Some(event) = event_receiver.borrow().clone() {
                        match event {
                            Event::SpendableOutputs { .. } => {
                                event::publish(&EventInternal::SpendableOutputs)
                            }
                            _ => tracing::trace!("Ignoring event on the mobile app"),
                        }
                    }
                }
                Err(_) => {
                    tracing::error!("Sender died, channel closed.");
                    break;
                }
            }
        }
    }
}
