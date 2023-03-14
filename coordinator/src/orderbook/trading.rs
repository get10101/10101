use crate::orderbook::routes::MatchParams;
use anyhow::Result;
use orderbook_commons::Order;
use orderbook_commons::OrderType;
use rust_decimal::Decimal;
use std::cmp::Ordering;
use trade::Direction;

/// Matches a provided market order with limit orders from the DB
///
/// If the order is a long order, we return the short orders sorted by price (highest first)
/// If the order is a short order, we return the long orders sorted by price (lowest first)
///
/// Note: `opposite_direction_orders` should contain only relevant orders. For safety this function
/// will filter it again though
pub fn match_order(
    order: Order,
    opposite_direction_orders: Vec<Order>,
) -> Result<Vec<MatchParams>> {
    if order.order_type == OrderType::Limit {
        // we don't match limit and limit at the moment
        return Ok(vec![]);
    }

    let opposite_direction_orders = opposite_direction_orders
        .into_iter()
        .filter(|o| !o.direction.eq(&order.direction))
        .collect();

    let is_long = order.direction == Direction::Long;
    let mut orders = sort_orders(opposite_direction_orders, is_long);

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
    use time::OffsetDateTime;
    use trade::Direction;
    use uuid::Uuid;

    fn dumm_long_order(price: Decimal, id: Uuid, quantity: Decimal) -> Order {
        Order {
            id,
            price,
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Long,
            quantity,
            order_type: OrderType::Limit,
            timestamp: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    pub fn when_short_then_sort_desc() {
        let order1 = dumm_long_order(dec!(20_000), Uuid::new_v4(), Default::default());
        let order2 = dumm_long_order(dec!(21_000), Uuid::new_v4(), Default::default());
        let order3 = dumm_long_order(dec!(20_500), Uuid::new_v4(), Default::default());

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, false);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order2);
    }

    #[test]
    pub fn when_long_then_sort_asc() {
        let order1 = dumm_long_order(dec!(20_000), Uuid::new_v4(), Default::default());
        let order2 = dumm_long_order(dec!(21_000), Uuid::new_v4(), Default::default());
        let order3 = dumm_long_order(dec!(20_500), Uuid::new_v4(), Default::default());

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, true);
        assert_eq!(orders[0], order2);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order1);
    }

    #[test]
    pub fn when_all_same_id_sort_by_id() {
        let order1 = dumm_long_order(dec!(20_000), Uuid::new_v4(), Default::default());
        let order2 = dumm_long_order(dec!(20_000), Uuid::new_v4(), Default::default());
        let order3 = dumm_long_order(dec!(20_000), Uuid::new_v4(), Default::default());

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
            dumm_long_order(dec!(20_000), Uuid::new_v4(), dec!(100)),
            dumm_long_order(dec!(21_000), Uuid::new_v4(), dec!(200)),
            dumm_long_order(dec!(20_000), Uuid::new_v4(), dec!(300)),
            dumm_long_order(dec!(22_000), Uuid::new_v4(), dec!(400)),
        ];

        let order = Order {
            id: Uuid::new_v4(),
            price: Default::default(),
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Short,
            quantity: dec!(100),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
        };

        let matched_orders = match_order(order, all_orders).unwrap();

        assert_eq!(matched_orders.len(), 1);
        let matched_order = matched_orders.get(0).unwrap();
        assert_eq!(matched_order.maker_order.quantity, dec!(100));
    }

    #[test]
    fn given_limit_and_market_with_smaller_amount_then_match_multiple() {
        let order1 = dumm_long_order(dec!(20_000), Uuid::new_v4(), dec!(100));
        let order2 = dumm_long_order(dec!(21_000), Uuid::new_v4(), dec!(200));
        let order3 = dumm_long_order(dec!(22_000), Uuid::new_v4(), dec!(400));
        let order4 = dumm_long_order(dec!(20_000), Uuid::new_v4(), dec!(300));
        let all_orders = vec![order1.clone(), order2, order3, order4.clone()];

        let order = Order {
            id: Uuid::new_v4(),
            price: Default::default(),
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Short,
            quantity: dec!(200),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
        };

        let matched_orders = match_order(order, all_orders).unwrap();

        assert_eq!(matched_orders.len(), 2);
        let matched_order = matched_orders.get(0).unwrap();
        assert_eq!(matched_order.maker_order.id, order1.id);
        let matched_order = matched_orders.get(1).unwrap();
        assert_eq!(matched_order.maker_order.id, order4.id);
    }

    #[test]
    fn given_long_when_needed_short_direction_then_no_match() {
        let all_orders = vec![
            dumm_long_order(dec!(20_000), Uuid::new_v4(), dec!(100)),
            dumm_long_order(dec!(21_000), Uuid::new_v4(), dec!(200)),
            dumm_long_order(dec!(22_000), Uuid::new_v4(), dec!(400)),
            dumm_long_order(dec!(20_000), Uuid::new_v4(), dec!(300)),
        ];

        let order = Order {
            id: Uuid::new_v4(),
            price: Default::default(),
            trader_id: "".to_string(),
            taken: false,
            direction: Direction::Long,
            quantity: dec!(200),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
        };

        let matched_orders = match_order(order, all_orders).unwrap();

        assert_eq!(matched_orders.len(), 0);
    }
}
