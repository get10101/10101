use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use parking_lot::Mutex;
use parking_lot::MutexGuard;
use state::Storage;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

static EVENT_HUB: Storage<Arc<Mutex<EventHub>>> = Storage::new();

pub(crate) fn get() -> MutexGuard<'static, EventHub> {
    EVENT_HUB
        .get_or_set(|| {
            Arc::new(Mutex::new(EventHub {
                subscribers: HashMap::new(),
            }))
        })
        .lock()
}

pub struct EventHub {
    subscribers: HashMap<EventType, Vec<Box<dyn Subscriber + 'static + Send + Sync>>>,
}

impl EventHub {
    /// Subscribes the subscriber to the events registered through the filter implementation. Note,
    /// that the filter hook will only be called once during the subscribe function and is not
    /// considered anymore when publishing.
    pub fn subscribe(&mut self, subscriber: impl Subscriber + 'static + Send + Sync + Clone) {
        for event_type in subscriber.events() {
            match self.subscribers.entry(event_type) {
                Entry::Vacant(e) => {
                    e.insert(vec![Box::new(subscriber.clone())]);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().push(Box::new(subscriber.clone()));
                }
            }
        }
    }

    /// Publishes the given event to all subscribers. Note, that this will be executed in a loop.
    pub fn publish(&self, event: &EventInternal) {
        tracing::debug!("Publishing event {:?}", event);
        if let Some(subscribers) = self.subscribers.get(&EventType::from(event.clone())) {
            for subscriber in subscribers {
                // todo: we should tokio spawn here.
                subscriber.notify(event);
            }
        }
    }
}
