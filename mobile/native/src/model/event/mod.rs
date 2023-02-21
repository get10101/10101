use crate::event::EventInternal;
use crate::ln_dlc::Balance;
use crate::model::order::Order;
use flutter_rust_bridge::frb;

pub mod flutter_subscriber;

#[frb]
#[derive(Clone)]
pub enum Event {
    Init(String),
    Log(String),
    OrderUpdateNotification(Order),
    // TODO: This balance should have it's own API type, at the moment we are sending out the
    // ln_dlc balance
    WalletInfo(Balance),
}

impl From<EventInternal> for Event {
    fn from(value: EventInternal) -> Self {
        match value {
            EventInternal::Init(value) => Event::Init(value),
            EventInternal::Log(value) => Event::Log(value),
            EventInternal::OrderUpdateNotification(value) => {
                Event::OrderUpdateNotification(value.into())
            }
            EventInternal::WalletInfo(value) => Event::WalletInfo(value),
        }
    }
}
