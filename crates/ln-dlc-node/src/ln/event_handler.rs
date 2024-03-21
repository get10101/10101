use anyhow::Result;
use lightning::events::Event;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::watch;

pub type EventSender = watch::Sender<Option<Event>>;

// Under non-WASM, we spawn tasks using tokio. Therefore, they must be Send + Sync
#[cfg(not(target_arch = "wasm32"))]
pub trait EventHandlerFuture<T>: Future<Output = T> + Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T, F> EventHandlerFuture<T> for F where F: Future<Output = T> + Send + Sync {}

// Under WASM, we spawn tasks using wasm_bindgen_futures, so Send + Sync is not required.
// Additionally, they _cannot_ be Send + Sync, as the esplora-client futures are not Send + Sync
// since they bind to JavaScript promises (which are not Send + Sync)
#[cfg(target_arch = "wasm32")]
pub trait EventHandlerFuture<T>: Future<Output = T> {}

#[cfg(target_arch = "wasm32")]
impl<T, F> EventHandlerFuture<T> for F where F: Future<Output = T> {}

pub trait EventHandlerTrait: Send + Sync {
    fn match_event(&self, event: Event) -> impl EventHandlerFuture<Result<()>>;

    fn handle_event(&self, event: Event) -> impl EventHandlerFuture<()> {
        async move {
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
    }

    fn event_sender(&self) -> &Option<watch::Sender<Option<Event>>> {
        &None
    }
}

pub mod handlers {}
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
