use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::node::Node;
use lightning::util::events::Event;
use tokio::sync::watch::Receiver;

impl Node {
    pub async fn listen_for_lightning_events(&self, mut event_receiver: Receiver<Option<Event>>) {
        loop {
            let event = match event_receiver.changed().await {
                Ok(()) => {
                    if let Some(event) = event_receiver.borrow().clone() {
                        event
                    } else {
                        continue;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to receive event: {e:#}");
                    continue;
                }
            };

            match event {
                Event::ChannelReady { channel_id, .. } => {
                    event::publish(&EventInternal::ChannelReady(channel_id))
                }
                Event::PaymentClaimed { amount_msat, .. } => {
                    event::publish(&EventInternal::PaymentClaimed(amount_msat))
                }
                _ => tracing::debug!("Ignoring event on the mobile app"),
            }
        }
    }
}
