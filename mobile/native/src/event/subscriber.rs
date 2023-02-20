use crate::event::Event;

pub trait Subscriber {
    fn notify(&self, event: &Event);
    fn filter(&self, event: &Event) -> bool;
}
