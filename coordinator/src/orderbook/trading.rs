use crate::orderbook::routes::MatchParams;
use anyhow::Result;
use orderbook_commons::Order;
use orderbook_commons::OrderType;
use rust_decimal::Decimal;
use std::cmp::Ordering;
use trade::Direction;

/// Matches a provided market order with limit orders from the DB
///
/// If the order is a long order, we return the orders with the highest price
/// If the order is a short order, we return the orders with the lowest price
pub fn match_order(order: Order, all_orders: Vec<Order>) -> Result<Vec<MatchParams>> {
    if order.order_type == OrderType::Limit {
        // we don't match limit and limit at the moment
        return Ok(vec![]);
    }

    let is_long = order.direction == Direction::Long;
    let mut orders = sort_orders(all_orders, is_long);

    let mut remaining_quantity = order.quantity;
    let mut matched_orders = vec![];
    while !orders.is_empty() {
        let matched_order = orders.remove(0);
        remaining_quantity -= matched_order.quantity;
        matched_orders.push(matched_order);

        if remaining_quantity <= Decimal::ZERO {
            break;
        }
    }

    Ok(matched_orders
        .iter()
        .map(|maker_order| MatchParams {
            // TODO: wait for the final type
            maker_order: maker_order.clone(),
            taker_order: order.clone(),
        })
        .collect())
}

/// sorts the provided list of orders
///
/// For matching market order and limit order we have to
/// - take the highest rate if the market order is long
/// - take the lowest rate if the market order is short
/// hence, we sort the orders here accordingly
/// - if long is needed: the resulting vec is ordered ascending.
/// - if short is needed: the resulting vec is ordered descending.
///
/// Note: if two orders have the same rate, we give the earlier order (the one with the lower id)
/// a higher ordering.
fn sort_orders(mut orders: Vec<Order>, is_long: bool) -> Vec<Order> {
    orders.sort_by(|a, b| {
        if a.price.cmp(&b.price) == Ordering::Equal {
            return a.id.cmp(&b.id);
        }
        if is_long {
            b.price.cmp(&a.price)
        } else {
            a.price.cmp(&b.price)
        }
    });
    orders
}

#[cfg(test)]
pub mod tests {
    use crate::orderbook::trading::match_order;
    use crate::orderbook::trading::sort_orders;
    use orderbook_commons::Order;
    use orderbook_commons::OrderType;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use trade::Direction;

    fn dummy_order(price: Decimal, id: i32, quantity: Decimal) -> Order {
        Order {
            id,
            price,
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Long,
            quantity,
            order_type: OrderType::Limit,
        }
    }

    #[test]
    pub fn when_short_then_sort_desc() {
        let order1 = dummy_order(dec!(20_000), 1, Default::default());
        let order2 = dummy_order(dec!(21_000), 2, Default::default());
        let order3 = dummy_order(dec!(20_500), 3, Default::default());

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, false);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order2);
    }

    #[test]
    pub fn when_long_then_sort_asc() {
        let order1 = dummy_order(dec!(20_000), 1, Default::default());
        let order2 = dummy_order(dec!(21_000), 2, Default::default());
        let order3 = dummy_order(dec!(20_500), 3, Default::default());

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, true);
        assert_eq!(orders[0], order2);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order1);
    }

    #[test]
    pub fn when_all_same_id_sort_by_id() {
        let order1 = dummy_order(dec!(20_000), 1, Default::default());
        let order2 = dummy_order(dec!(20_000), 2, Default::default());
        let order3 = dummy_order(dec!(20_000), 3, Default::default());

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, true);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order2);
        assert_eq!(orders[2], order3);

        let orders = sort_orders(orders, false);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order2);
        assert_eq!(orders[2], order3);
    }

    #[test]
    fn given_limit_and_market_with_same_amount_then_match() {
        let all_orders = vec![
            dummy_order(dec!(20_000), 1, dec!(100)),
            dummy_order(dec!(21_000), 2, dec!(200)),
            dummy_order(dec!(20_000), 3, dec!(300)),
            dummy_order(dec!(22_000), 4, dec!(400)),
        ];

        let order = Order {
            id: 1,
            price: Default::default(),
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Short,
            quantity: dec!(100),
            order_type: OrderType::Market,
        };

        let matched_orders = match_order(order, all_orders).unwrap();

        assert_eq!(matched_orders.len(), 1);
        let matched_order = matched_orders.get(0).unwrap();
        assert_eq!(matched_order.maker_order.quantity, dec!(100));
    }

    #[test]
    fn given_limit_and_market_with_smaller_amount_then_match_multiple() {
        let all_orders = vec![
            dummy_order(dec!(20_000), 1, dec!(100)),
            dummy_order(dec!(21_000), 2, dec!(200)),
            dummy_order(dec!(22_000), 3, dec!(400)),
            dummy_order(dec!(20_000), 4, dec!(300)),
        ];

        let order = Order {
            id: 1,
            price: Default::default(),
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Short,
            quantity: dec!(200),
            order_type: OrderType::Market,
        };

        let matched_orders = match_order(order, all_orders).unwrap();

        assert_eq!(matched_orders.len(), 2);
        let matched_order = matched_orders.get(0).unwrap();
        assert_eq!(matched_order.maker_order.id, 1);
        let matched_order = matched_orders.get(1).unwrap();
        assert_eq!(matched_order.maker_order.id, 4);
    }
}
