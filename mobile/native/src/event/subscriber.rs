use crate::event::{EventInternal, EventType};

pub trait Subscriber {
    fn notify(&self, event: &EventInternal);
    fn events(&self) -> Vec<EventType>;
}
