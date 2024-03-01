use crate::order::Order;
use crate::order::OrderState;
use crate::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Price {
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
}

pub type Prices = HashMap<ContractSymbol, Price>;

/// Best prices across all current orders for given ContractSymbol in the orderbook
/// Taken orders are not included in the average
pub fn best_current_price(current_orders: &[Order]) -> Prices {
    let mut prices = HashMap::new();
    let mut add_price_for_symbol = |symbol| {
        prices.insert(
            symbol,
            Price {
                bid: best_bid_price(current_orders, symbol),
                ask: best_ask_price(current_orders, symbol),
            },
        );
    };
    add_price_for_symbol(ContractSymbol::BtcUsd);
    prices
}

/// Best price (highest) of all long (buy) orders in the orderbook
fn best_bid_price(current_orders: &[Order], symbol: ContractSymbol) -> Option<Decimal> {
    best_price_for(current_orders, Direction::Long, symbol)
}

/// Best price (lowest) of all short (sell) orders in the orderbook
fn best_ask_price(current_orders: &[Order], symbol: ContractSymbol) -> Option<Decimal> {
    best_price_for(current_orders, Direction::Short, symbol)
}

fn best_price_for(
    current_orders: &[Order],
    direction: Direction,
    symbol: ContractSymbol,
) -> Option<Decimal> {
    assert_eq!(
        symbol,
        ContractSymbol::BtcUsd,
        "only btcusd supported for now"
    );
    let use_max = direction == Direction::Long;
    current_orders
        .iter()
        .filter(|order| order.order_state == OrderState::Open && order.direction == direction)
        .map(|order| order.price.to_f64().expect("to represent decimal as f64"))
        // get the best price
        .fold(None, |acc, x| match acc {
            Some(y) => Some(if use_max { x.max(y) } else { x.min(y) }),
            None => Some(x),
        })?
        .try_into()
        .ok()
}

#[cfg(test)]
mod test {
    use crate::order::Order;
    use crate::order::OrderReason;
    use crate::order::OrderState;
    use crate::order::OrderType;
    use crate::price::best_ask_price;
    use crate::price::best_bid_price;
    use bitcoin::secp256k1::PublicKey;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use trade::ContractSymbol;
    use trade::Direction;
    use uuid::Uuid;
    use ContractSymbol::BtcUsd;

    fn dummy_public_key() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .unwrap()
    }

    fn dummy_order(price: Decimal, direction: Direction, order_state: OrderState) -> Order {
        Order {
            id: Uuid::new_v4(),
            price,
            trader_id: dummy_public_key(),
            direction,
            leverage: 1.0,
            contract_symbol: BtcUsd,
            quantity: 100.into(),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
            expiry: OffsetDateTime::now_utc(),
            order_state,
            order_reason: OrderReason::Manual,
            stable: false,
        }
    }

    #[test]
    fn test_best_bid_price() {
        let current_orders = vec![
            dummy_order(dec!(10_000), Direction::Long, OrderState::Open),
            dummy_order(dec!(30_000), Direction::Long, OrderState::Open),
            dummy_order(dec!(500_000), Direction::Long, OrderState::Taken), // taken
            dummy_order(dec!(50_000), Direction::Short, OrderState::Open),  // wrong direction
        ];
        assert_eq!(best_bid_price(&current_orders, BtcUsd), Some(dec!(30_000)));
    }

    #[test]
    fn test_best_ask_price() {
        let current_orders = vec![
            dummy_order(dec!(10_000), Direction::Short, OrderState::Open),
            dummy_order(dec!(30_000), Direction::Short, OrderState::Open),
            // ignored in the calculations - this order is taken
            dummy_order(dec!(5_000), Direction::Short, OrderState::Taken),
            // ignored in the calculations - it's the bid price
            dummy_order(dec!(50_000), Direction::Long, OrderState::Open),
        ];
        assert_eq!(best_ask_price(&current_orders, BtcUsd), Some(dec!(10_000)));
    }

    #[test]
    fn test_no_price() {
        let all_orders_taken = vec![
            dummy_order(dec!(10_000), Direction::Short, OrderState::Taken),
            dummy_order(dec!(30_000), Direction::Long, OrderState::Taken),
        ];

        assert_eq!(best_ask_price(&all_orders_taken, BtcUsd), None);
        assert_eq!(best_bid_price(&all_orders_taken, BtcUsd), None);
    }
}
