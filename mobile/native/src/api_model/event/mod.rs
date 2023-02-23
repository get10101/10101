use crate::api::WalletInfo;
use crate::api_model::order::Order;
use crate::api_model::position::Position;
use crate::event::EventInternal;
use flutter_rust_bridge::frb;

pub mod flutter_subscriber;

#[frb]
#[derive(Clone)]
pub enum Event {
    Init(String),
    Log(String),
    OrderUpdateNotification(Order),
    WalletInfoUpdateNotification(WalletInfo),
    PositionUpdateNotification(Position),
}

impl From<EventInternal> for Event {
    fn from(value: EventInternal) -> Self {
        match value {
            EventInternal::Init(value) => Event::Init(value),
            EventInternal::Log(value) => Event::Log(value),
            EventInternal::OrderUpdateNotification(value) => {
                Event::OrderUpdateNotification(value.into())
            }
            EventInternal::WalletInfoUpdateNotification(value) => {
                Event::WalletInfoUpdateNotification(value)
            }
            EventInternal::OrderFilledWith(_) => {
                unreachable!("This internal event is not exposed to the UI")
            }
            EventInternal::PositionUpdateNotification(position) => {
                Event::PositionUpdateNotification(position.into())
            }
        }
    }
}
