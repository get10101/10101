use crate::event::EventInternal;

pub trait Subscriber {
    fn notify(&self, event: &EventInternal);
    fn filter(&self, event: &EventInternal) -> bool;
}
