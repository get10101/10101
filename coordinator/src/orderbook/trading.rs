use crate::orderbook::routes::MatchParams;
use crate::orderbook::routes::TraderMatchParams;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::XOnlyPublicKey;
use orderbook_commons::Match;
use orderbook_commons::Order;
use orderbook_commons::OrderType;
use orderbook_commons::{FilledWith, OrderbookMsg};
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::mpsc::Sender;
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
) -> Result<Option<MatchParams>> {
    if order.order_type == OrderType::Limit {
        // we don't match limit and limit at the moment
        return Ok(None);
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

    // For the time being we do not want to support multi match
    if matched_orders.len() > 1 {
        bail!("More than one matched order, please reduce order quantity");
    }

    if matched_orders.is_empty() {
        return Ok(None);
    }

    // For now we go for 1 week contracts, this has been chosen randomly and should be chosen wisely
    // once we move to perpetuals
    let expiry_timestamp = OffsetDateTime::now_utc() + Duration::days(7);

    // For now we hardcode the oracle pubkey here
    let oracle_pk = XOnlyPublicKey::from_str(
        "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0",
    )
    .expect("To be a valid pubkey");

    let matches = matched_orders
        .iter()
        .map(|maker_order| {
            (
                TraderMatchParams {
                    trader_id: maker_order.trader_id,
                    filled_with: FilledWith {
                        order_id: maker_order.id,
                        expiry_timestamp,
                        oracle_pk,
                        matches: vec![Match {
                            order_id: order.id,
                            quantity: order.quantity,
                            pubkey: order.trader_id,
                            execution_price: maker_order.price,
                        }],
                    },
                },
                Match {
                    order_id: maker_order.id,
                    quantity: order.quantity,
                    pubkey: maker_order.trader_id,
                    execution_price: maker_order.price,
                },
            )
        })
        .collect::<Vec<(TraderMatchParams, Match)>>();

    let mut maker_matches = vec![];
    let mut taker_matches = vec![];

    for (mm, taker_match) in matches {
        maker_matches.push(mm);
        taker_matches.push(taker_match);
    }

    Ok(Some(MatchParams {
        taker_matches: TraderMatchParams {
            trader_id: order.trader_id,
            filled_with: FilledWith {
                order_id: order.id,
                expiry_timestamp,
                oracle_pk,
                matches: taker_matches,
            },
        },
        makers_matches: maker_matches,
    }))
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
/// Note: if two orders have the same rate, we give the earlier order
/// a higher ordering.
fn sort_orders(mut orders: Vec<Order>, is_long: bool) -> Vec<Order> {
    orders.sort_by(|a, b| {
        if a.price.cmp(&b.price) == Ordering::Equal {
            return a.timestamp.cmp(&b.timestamp);
        }
        if is_long {
            b.price.cmp(&a.price)
        } else {
            a.price.cmp(&b.price)
        }
    });
    orders
}

pub async fn notify_traders(
    matched_orders: MatchParams,
    authenticated_users: HashMap<PublicKey, Sender<OrderbookMsg>>,
) {
    for maker_match in matched_orders.makers_matches {
        match authenticated_users.get(&maker_match.trader_id) {
            None => {
                // TODO we should fail here and get another match if possible
                tracing::error!("Could not notify maker - we should fail here and get another match if possible");
            }
            Some(sender) => match sender
                .send(OrderbookMsg::Match(maker_match.filled_with))
                .await
            {
                Ok(_) => {
                    tracing::debug!("Successfully notified maker")
                }
                Err(err) => {
                    tracing::error!("Connection lost to maker {err:#}")
                }
            },
        }
    }
    match authenticated_users.get(&matched_orders.taker_matches.trader_id) {
        None => {
            // TODO we should fail here and get another match if possible
            tracing::error!(
                "Could not notify taker - we should fail here and get another match if possible"
            );
        }
        Some(sender) => match sender
            .send(OrderbookMsg::Match(
                matched_orders.taker_matches.filled_with,
            ))
            .await
        {
            Ok(_) => {
                tracing::debug!("Successfully notified taker")
            }
            Err(err) => {
                // TODO we should fail here and get another match if possible
                tracing::error!("Connection lost to taker {err:#}")
            }
        },
    }
}

#[cfg(test)]
pub mod tests {
    use crate::orderbook::trading::match_order;
    use crate::orderbook::trading::sort_orders;
    use bitcoin::secp256k1::PublicKey;
    use orderbook_commons::Order;
    use orderbook_commons::OrderType;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::Duration;
    use time::OffsetDateTime;
    use trade::Direction;
    use uuid::Uuid;

    fn dumm_long_order(
        price: Decimal,
        id: Uuid,
        quantity: Decimal,
        timestamp_delay: Duration,
    ) -> Order {
        Order {
            id,
            price,
            trader_id: PublicKey::from_str(
                "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
            )
            .unwrap(),
            taken: false,
            direction: Direction::Long,
            quantity,
            order_type: OrderType::Limit,
            timestamp: OffsetDateTime::now_utc() + timestamp_delay,
        }
    }

    #[test]
    pub fn when_short_then_sort_desc() {
        let order1 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order2 = dumm_long_order(
            dec!(21_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order3 = dumm_long_order(
            dec!(20_500),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, false);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order2);
    }

    #[test]
    pub fn when_long_then_sort_asc() {
        let order1 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order2 = dumm_long_order(
            dec!(21_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order3 = dumm_long_order(
            dec!(20_500),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, true);
        assert_eq!(orders[0], order2);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order1);
    }

    #[test]
    pub fn when_all_same_price_sort_by_id() {
        let order1 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order2 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(1),
        );
        let order3 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(2),
        );

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
            dumm_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(100),
                Duration::seconds(0),
            ),
            dumm_long_order(
                dec!(21_000),
                Uuid::new_v4(),
                dec!(200),
                Duration::seconds(0),
            ),
            dumm_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(300),
                Duration::seconds(0),
            ),
            dumm_long_order(
                dec!(22_000),
                Uuid::new_v4(),
                dec!(400),
                Duration::seconds(0),
            ),
        ];

        let order = Order {
            id: Uuid::new_v4(),
            price: Default::default(),
            trader_id: PublicKey::from_str(
                "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
            )
            .unwrap(),
            taken: false,
            direction: Direction::Short,
            quantity: dec!(100),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
        };

        let matched_orders = match_order(order.clone(), all_orders).unwrap().unwrap();

        assert_eq!(matched_orders.makers_matches.len(), 1);
        let maker_matches = matched_orders
            .makers_matches
            .get(0)
            .unwrap()
            .filled_with
            .matches
            .clone();
        assert_eq!(maker_matches.len(), 1);
        assert_eq!(maker_matches.get(0).unwrap().quantity, dec!(100));

        assert_eq!(matched_orders.taker_matches.filled_with.order_id, order.id);
        assert_eq!(matched_orders.taker_matches.filled_with.matches.len(), 1);
        assert_eq!(
            matched_orders
                .taker_matches
                .filled_with
                .matches
                .get(0)
                .unwrap()
                .quantity,
            order.quantity
        );
    }

    /// This test is for safety reasons only. Once we want multiple matches we should update it
    #[test]
    fn given_limit_and_market_with_smaller_amount_then_error() {
        let order1 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            dec!(100),
            Duration::seconds(0),
        );
        let order2 = dumm_long_order(
            dec!(21_000),
            Uuid::new_v4(),
            dec!(200),
            Duration::seconds(0),
        );
        let order3 = dumm_long_order(
            dec!(22_000),
            Uuid::new_v4(),
            dec!(400),
            Duration::seconds(0),
        );
        let order4 = dumm_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            dec!(300),
            Duration::seconds(0),
        );
        let all_orders = vec![order1, order2, order3, order4];

        let order = Order {
            id: Uuid::new_v4(),
            price: Default::default(),
            trader_id: PublicKey::from_str(
                "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
            )
            .unwrap(),
            taken: false,
            direction: Direction::Short,
            quantity: dec!(200),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
        };

        assert!(match_order(order, all_orders).is_err());
    }

    #[test]
    fn given_long_when_needed_short_direction_then_no_match() {
        let all_orders = vec![
            dumm_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(100),
                Duration::seconds(0),
            ),
            dumm_long_order(
                dec!(21_000),
                Uuid::new_v4(),
                dec!(200),
                Duration::seconds(0),
            ),
            dumm_long_order(
                dec!(22_000),
                Uuid::new_v4(),
                dec!(400),
                Duration::seconds(0),
            ),
            dumm_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(300),
                Duration::seconds(0),
            ),
        ];

        let order = Order {
            id: Uuid::new_v4(),
            price: Default::default(),
            trader_id: PublicKey::from_str(
                "027f31ebc5462c1fdce1b737ecff52d37d75dea43ce11c74d25aa297165faa2007",
            )
            .unwrap(),
            taken: false,
            direction: Direction::Long,
            quantity: dec!(200),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
        };

        let matched_orders = match_order(order, all_orders).unwrap();

        assert!(matched_orders.is_none());
    }
}
