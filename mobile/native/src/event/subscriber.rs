use crate::event::EventInternal;
use crate::event::EventType;

pub trait Subscriber {
    /// Notifies the subcriber about an event. If false is returned the subscriber will be
    /// unsubscribed, if true the subscriber will remain subscribed.
    fn notify(&self, event: &EventInternal);

    /// Returns a list of events the subscriber wants to subscribe to.
    fn events(&self) -> Vec<EventType>;
}
