use crate::event::EventInternal;
use crate::event::EventType;

pub trait Subscriber {
    fn notify(&self, event: &EventInternal);
    fn events(&self) -> Vec<EventType>;
}
