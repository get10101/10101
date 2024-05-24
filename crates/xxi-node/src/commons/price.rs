use crate::commons::order::Order;
use crate::commons::order::OrderState;
use crate::commons::ContractSymbol;
use crate::commons::Direction;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Price {
    pub bid: Decimal,
    pub ask: Decimal,
}

impl Price {
    /// Get the price for the direction
    ///
    /// For going long we get the best ask price, for going short we get the best bid price.
    pub fn get_price_for_direction(&self, direction: Direction) -> Decimal {
        match direction {
            Direction::Long => self.ask,
            Direction::Short => self.bid,
        }
    }
}

pub type Prices = HashMap<ContractSymbol, BestPrice>;

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct BestPrice {
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
}

/// Best prices across all current orders for given ContractSymbol in the orderbook
/// Taken orders are not included in the average
pub fn best_current_price(current_orders: &[Order]) -> Prices {
    let mut prices = HashMap::new();
    let mut add_price_for_symbol = |symbol| {
        prices.insert(
            symbol,
            BestPrice {
                bid: best_bid_price(current_orders, symbol),
                ask: best_ask_price(current_orders, symbol),
            },
        );
    };
    add_price_for_symbol(ContractSymbol::BtcUsd);
    prices
}

/// If you place a market order to go short/sell, the best/highest `Bid` price
///
/// Differently said, remember `buy high`, `sell low`!
/// Ask = high
/// Bid = low
///
/// The best `Ask` is the lowest of all `Asks`
/// The best `Bid` is the highest of all `Bids`
///
/// If you SELL, you ask and you get the best price someone is willing to buy at i.e. the highest
/// bid price.
pub fn best_bid_price(orders: &[Order], symbol: ContractSymbol) -> Option<Decimal> {
    orders
        .iter()
        .filter(|o| {
            o.order_state == OrderState::Open
                && o.direction == Direction::Long
                && o.contract_symbol == symbol
                && o.expiry > OffsetDateTime::now_utc()
        })
        .map(|o| o.price)
        .max()
}

/// If you place a market order to go long/buy, you get the best/lowest `Ask` price
///
/// Differently said, remember `buy high`, `sell low`!
/// Ask = high
/// Bid = low
///
/// The best `Ask` is the lowest of all `Asks`
/// The best `Bid` is the highest of all `Bids`
///
/// If you BUY, you bid and you get the best price someone is willing to sell at i.e. the lowest ask
/// price.
pub fn best_ask_price(orders: &[Order], symbol: ContractSymbol) -> Option<Decimal> {
    orders
        .iter()
        .filter(|o| {
            o.order_state == OrderState::Open
                && o.direction == Direction::Short
                && o.contract_symbol == symbol
                && o.expiry > OffsetDateTime::now_utc()
        })
        .map(|o| o.price)
        .min()
}

#[cfg(test)]
mod test {
    use crate::commons::order::Order;
    use crate::commons::order::OrderReason;
    use crate::commons::order::OrderState;
    use crate::commons::order::OrderType;
    use crate::commons::price::best_ask_price;
    use crate::commons::price::best_bid_price;
    use crate::commons::ContractSymbol;
    use crate::commons::Direction;
    use bitcoin::secp256k1::PublicKey;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::Duration;
    use time::OffsetDateTime;
    use uuid::Uuid;

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
            contract_symbol: ContractSymbol::BtcUsd,
            quantity: 100.into(),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
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
        assert_eq!(
            best_bid_price(&current_orders, ContractSymbol::BtcUsd),
            Some(dec!(30_000))
        );
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
        assert_eq!(
            best_ask_price(&current_orders, ContractSymbol::BtcUsd),
            Some(dec!(10_000))
        );
    }

    #[test]
    fn test_no_price() {
        let all_orders_taken = vec![
            dummy_order(dec!(10_000), Direction::Short, OrderState::Taken),
            dummy_order(dec!(30_000), Direction::Long, OrderState::Taken),
        ];

        assert_eq!(
            best_ask_price(&all_orders_taken, ContractSymbol::BtcUsd),
            None
        );
        assert_eq!(
            best_bid_price(&all_orders_taken, ContractSymbol::BtcUsd),
            None
        );
    }
}
