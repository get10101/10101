pub mod api;
mod event_hub;
pub mod subscriber;

use crate::api::WalletInfo;
use std::hash::Hash;
use std::hash::Hasher;
use strum_macros::EnumIter;

use crate::event::event_hub::get;
use crate::event::subscriber::Subscriber;
use crate::trade::order::OrderTrade;
use crate::trade::position::PositionTrade;
use crate::trade::position::TradeParams;

pub fn subscribe(subscriber: impl Subscriber + 'static + Send + Sync + Clone) {
    get().subscribe(subscriber);
}

pub fn publish(event: &EventInternal) {
    get().publish(event);
}

#[derive(Clone, EnumIter, Debug)]
pub enum EventInternal {
    Init(String),
    Log(String),
    OrderUpdateNotification(OrderTrade),
    WalletInfoUpdateNotification(WalletInfo),
    OrderFilledWith(TradeParams),
    PositionUpdateNotification(PositionTrade),
}

impl PartialEq for EventInternal {
    fn eq(&self, other: &Self) -> bool {
        // Values are considered equal just by the enum variant, ignoring the values, see: https://stackoverflow.com/questions/32554285/compare-enums-only-by-variant-not-value
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl Hash for EventInternal {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        // Values are hashed based on enum variant, ignoring the values, see: https://stackoverflow.com/questions/32554285/compare-enums-only-by-variant-not-value
        std::mem::discriminant(self).hash(hasher)
    }
}

impl Eq for EventInternal {}

#[cfg(test)]
mod tests {
    use crate::api::Balances;
    use crate::api::WalletInfo;
    use crate::event::EventInternal;
    use std::collections::HashMap;

    #[test]
    fn given_log_events_with_different_values_when_comparing_with_equals_then_is_equal() {
        let event1 = EventInternal::Init("satoshi".to_string());
        let event2 = EventInternal::Init("rulz".to_string());
        assert_eq!(event1, event2)
    }

    #[test]
    fn given_log_event_with_different_values_when_used_as_key_in_hashmap_then_is_treated_as_same_key(
    ) {
        let event1 = EventInternal::Init("satoshi".to_string());
        let event2 = EventInternal::Init("rulz".to_string());

        let mut map = HashMap::new();

        map.insert(event1.clone(), "big".to_string());
        assert_eq!(*map.get(&event1).unwrap(), "big".to_string());
        assert_eq!(*map.get(&event2).unwrap(), "big".to_string());

        map.insert(event2.clone(), "time".to_string());
        assert_eq!(*map.get(&event1).unwrap(), "time".to_string());
        assert_eq!(*map.get(&event2).unwrap(), "time".to_string());
    }

    #[test]
    fn given_wallet_info_event_with_different_balances_when_used_as_key_in_hashmap_then_is_treated_as_same_key(
    ) {
        let event1 = EventInternal::WalletInfoUpdateNotification(WalletInfo {
            balances: Balances {
                on_chain: 1,
                lightning: 1,
            },
            history: vec![],
        });
        let event2 = EventInternal::WalletInfoUpdateNotification(WalletInfo {
            balances: Balances {
                on_chain: 2,
                lightning: 2,
            },
            history: vec![],
        });

        let mut map = HashMap::new();

        map.insert(event1.clone(), "big".to_string());
        assert_eq!(*map.get(&event1).unwrap(), "big".to_string());
        assert_eq!(*map.get(&event2).unwrap(), "big".to_string());

        map.insert(event2.clone(), "time".to_string());
        assert_eq!(*map.get(&event1).unwrap(), "time".to_string());
        assert_eq!(*map.get(&event2).unwrap(), "time".to_string());
    }
}
