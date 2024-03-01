use anyhow::Result;
use async_trait::async_trait;
use lightning::events::Event;
use std::sync::Arc;
use tokio::sync::watch;

pub type EventSender = watch::Sender<Option<Event>>;

#[async_trait]
pub trait EventHandlerTrait: Send + Sync {
    async fn match_event(&self, event: Event) -> Result<()>;

    async fn handle_event(&self, event: Event) {
        tracing::debug!(?event, "Received event");

        let event_str = format!("{event:?}");

        match self.match_event(event.clone()).await {
            Ok(()) => tracing::debug!(event = ?event_str, "Successfully handled event"),
            Err(e) => tracing::error!("Failed to handle event. Error: {e:#}"),
        }

        if let Some(event_sender) = self.event_sender() {
            match event_sender.send(Some(event)) {
                Ok(()) => tracing::trace!("Sent event to subscriber"),
                Err(e) => tracing::error!("Failed to send event to subscriber: {e:#}"),
            }
        }
    }

    fn event_sender(&self) -> &Option<watch::Sender<Option<Event>>> {
        &None
    }
}

pub mod handlers {}
#[async_trait]
impl<T: EventHandlerTrait + ?Sized> EventHandlerTrait for Arc<T> {
    async fn match_event(&self, event: Event) -> Result<()> {
        (**self).match_event(event).await
    }

    async fn handle_event(&self, event: Event) {
        (**self).handle_event(event).await
    }

    fn event_sender(&self) -> &Option<watch::Sender<Option<Event>>> {
        (**self).event_sender()
    }
}
