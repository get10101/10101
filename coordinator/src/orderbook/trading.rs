use crate::orderbook;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use bitcoin::XOnlyPublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use orderbook_commons::FilledWith;
use orderbook_commons::Match;
use orderbook_commons::NewOrder;
use orderbook_commons::Order;
use orderbook_commons::OrderType;
use orderbook_commons::OrderbookMsg;
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use trade::Direction;

pub struct Trading {
    pool: Pool<ConnectionManager<PgConnection>>,
    authenticated_users: HashMap<PublicKey, mpsc::Sender<OrderbookMsg>>,
    receiver: mpsc::Receiver<TradingMessage>,
    tx_price_feed: broadcast::Sender<OrderbookMsg>,
}

pub enum TradingMessage {
    NewOrder(NewOrderMessage),
    NewUser(NewUserMessage),
}

pub struct NewOrderMessage {
    pub new_order: NewOrder,
    pub sender: mpsc::Sender<Result<Order>>,
}

pub struct NewUserMessage {
    pub new_user: PublicKey,
    pub sender: mpsc::Sender<OrderbookMsg>,
}

#[derive(Error, Debug, PartialEq)]
pub enum TradingError {
    #[error("Invalid order: {0}")]
    InvalidOrder(String),
    #[error("{0}")]
    NoMatchFound(String),
}

#[derive(Clone)]
pub struct MatchParams {
    pub taker_matches: TraderMatchParams,
    pub makers_matches: Vec<TraderMatchParams>,
}

#[derive(Clone)]
pub struct TraderMatchParams {
    pub trader_id: PublicKey,
    pub filled_with: FilledWith,
}

impl Trading {
    /// starts the trading task and returns a sender that can be used to send `TradingMessages` to
    /// the trading task.
    pub fn start(
        pool: Pool<ConnectionManager<PgConnection>>,
        tx_price_feed: broadcast::Sender<OrderbookMsg>,
    ) -> (JoinHandle<Result<()>>, mpsc::Sender<TradingMessage>) {
        let (sender, receiver) = mpsc::channel::<TradingMessage>(100);

        let handle = Trading {
            pool,
            authenticated_users: HashMap::new(),
            receiver,
            tx_price_feed,
        }
        .spawn();

        (handle, sender)
    }

    /// spawns a new tokio task that is handling messages
    fn spawn(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            while let Some(trading_message) = self.receiver.recv().await {
                match trading_message {
                    TradingMessage::NewOrder(new_order_msg) => {
                        let result = self.process_new_order(new_order_msg.new_order).await;
                        new_order_msg.sender.send(result).await?;
                    }
                    TradingMessage::NewUser(new_user_msg) => {
                        self.authenticated_users
                            .insert(new_user_msg.new_user, new_user_msg.sender);
                    }
                }
            }

            Ok(())
        })
    }

    /// Processes a new limit and market order
    ///
    /// Limit order: update price feed
    /// Market order: find match and notify traders
    async fn process_new_order(&self, new_order: NewOrder) -> Result<Order> {
        tracing::info!(trader_id=%new_order.trader_id, "Received a new {:?} order", new_order.order_type);

        if new_order.order_type == OrderType::Limit && new_order.price == Decimal::ZERO {
            return Err(TradingError::InvalidOrder(
                "Limit order with zero price are not allowed".to_string(),
            ))?;
        }

        let mut conn = self.pool.get()?;
        let order = orderbook::db::orders::insert(&mut conn, new_order.clone())
            .map_err(|e| anyhow!("Failed to insert new order into db: {e:#}"))?;

        if new_order.order_type == OrderType::Limit {
            // we only tell everyone about new limit orders
            self.tx_price_feed
                .send(OrderbookMsg::NewOrder(order.clone()))
                .map_err(|error| anyhow!("Could not update price feed due to '{error}'"))?;
        } else {
            let mut connection = self.pool.clone().get()?;
            let opposite_direction_orders = orderbook::db::orders::all_by_direction_and_type(
                &mut connection,
                order.direction.opposite(),
                OrderType::Limit,
                false,
                true,
            )?;

            let matched_orders = match_order(&order, opposite_direction_orders)?
                .with_context(|| format!("Could not match order {}", order.id))
                .map_err(|e| TradingError::NoMatchFound(format!("{e:#}")))?;

            tracing::info!(order_id=%order.id, trader_id=%order.trader_id, "Found a match with {} makers for new order.", matched_orders.makers_matches.len());

            let mut orders_to_set_taken = vec![matched_orders.taker_matches.filled_with.order_id];
            let mut order_ids = matched_orders
                .taker_matches
                .filled_with
                .matches
                .iter()
                .map(|m| m.order_id)
                .collect();

            orders_to_set_taken.append(&mut order_ids);
            notify_traders(matched_orders, &self.authenticated_users).await?;

            for order_id in orders_to_set_taken {
                tracing::info!(%order_id, "Setting order to taken.");
                if let Err(err) = orderbook::db::orders::set_is_taken(&mut conn, order_id, true) {
                    let order_id = order_id.to_string();
                    tracing::error!(order_id, "Could not set order to taken. Error: {err:#}");
                }
            }
        }

        Ok(order)
    }
}

/// Matches a provided market order with limit orders from the DB
///
/// If the order is a long order, we return the short orders sorted by price (highest first)
/// If the order is a short order, we return the long orders sorted by price (lowest first)
///
/// Note: `opposite_direction_orders` should contain only relevant orders. For safety this function
/// will filter it again though
#[autometrics]
pub fn match_order(
    order: &Order,
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

    let tomorrow = OffsetDateTime::now_utc().date() + Duration::days(7);
    let expiry_timestamp = tomorrow.midnight().assume_utc();

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
/// - take the highest rate if the market order is short
/// - take the lowest rate if the market order is long
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
            a.price.cmp(&b.price)
        } else {
            b.price.cmp(&a.price)
        }
    });
    orders
}

/// Notifies all participating traders about a match.
//
/// FIXME: This logic currently does not care handle the scenario if the trader is not online at
/// the time of match.
pub async fn notify_traders(
    matched_orders: MatchParams,
    traders: &HashMap<PublicKey, mpsc::Sender<OrderbookMsg>>,
) -> Result<()> {
    for maker_match in matched_orders.makers_matches {
        let maker_id = maker_match.trader_id.to_string();
        match notify_trader(&maker_match, traders).await {
            Ok(()) => tracing::debug!(maker_id, "Successfully notified maker"),
            Err(e) => tracing::warn!(maker_id, "{e:#}"),
        }
    }

    let taker_id = matched_orders.taker_matches.trader_id.to_string();
    match notify_trader(&matched_orders.taker_matches, traders).await {
        Ok(()) => tracing::debug!(taker_id, "Successfully notified taker"),
        Err(e) => tracing::warn!(taker_id, "{e:#}"),
    }

    Ok(())
}

async fn notify_trader(
    match_params: &TraderMatchParams,
    traders: &HashMap<PublicKey, mpsc::Sender<OrderbookMsg>>,
) -> Result<()> {
    match traders.get(&match_params.trader_id) {
        None => bail!("Trader is not connected"),
        Some(sender) => sender
            .send(OrderbookMsg::Match(match_params.filled_with.clone()))
            .await
            .map_err(|err| anyhow!("Connection lost to trader {err:#}")),
    }
}

#[cfg(test)]
pub mod tests {
    use crate::orderbook::trading::match_order;
    use crate::orderbook::trading::notify_traders;
    use crate::orderbook::trading::sort_orders;
    use crate::orderbook::trading::MatchParams;
    use crate::orderbook::trading::TraderMatchParams;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::secp256k1::SecretKey;
    use bitcoin::secp256k1::SECP256K1;
    use bitcoin::XOnlyPublicKey;
    use orderbook_commons::FilledWith;
    use orderbook_commons::Match;
    use orderbook_commons::Order;
    use orderbook_commons::OrderType;
    use orderbook_commons::OrderbookMsg;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use std::str::FromStr;
    use time::Duration;
    use time::OffsetDateTime;
    use tokio::sync::mpsc;
    use trade::Direction;
    use uuid::Uuid;

    fn dummy_long_order(
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
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
        }
    }

    #[test]
    pub fn when_short_then_sort_desc() {
        let order1 = dummy_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order2 = dummy_long_order(
            dec!(21_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order3 = dummy_long_order(
            dec!(20_500),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, false);
        assert_eq!(orders[0], order2);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order1);
    }

    #[test]
    pub fn when_long_then_sort_asc() {
        let order1 = dummy_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order2 = dummy_long_order(
            dec!(21_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order3 = dummy_long_order(
            dec!(20_500),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );

        let orders = vec![order3.clone(), order1.clone(), order2.clone()];

        let orders = sort_orders(orders, true);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order2);
    }

    #[test]
    pub fn when_all_same_price_sort_by_id() {
        let order1 = dummy_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(0),
        );
        let order2 = dummy_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            Default::default(),
            Duration::seconds(1),
        );
        let order3 = dummy_long_order(
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
            dummy_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(100),
                Duration::seconds(0),
            ),
            dummy_long_order(
                dec!(21_000),
                Uuid::new_v4(),
                dec!(200),
                Duration::seconds(0),
            ),
            dummy_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(300),
                Duration::seconds(0),
            ),
            dummy_long_order(
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
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
        };

        let matched_orders = match_order(&order, all_orders).unwrap().unwrap();

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
        let order1 = dummy_long_order(
            dec!(20_000),
            Uuid::new_v4(),
            dec!(400),
            Duration::seconds(0),
        );
        let order2 = dummy_long_order(
            dec!(21_000),
            Uuid::new_v4(),
            dec!(200),
            Duration::seconds(0),
        );
        let order3 = dummy_long_order(
            dec!(22_000),
            Uuid::new_v4(),
            dec!(100),
            Duration::seconds(0),
        );
        let order4 = dummy_long_order(
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
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
        };

        assert!(match_order(&order, all_orders).is_err());
    }

    #[test]
    fn given_long_when_needed_short_direction_then_no_match() {
        let all_orders = vec![
            dummy_long_order(
                dec!(20_000),
                Uuid::new_v4(),
                dec!(100),
                Duration::seconds(0),
            ),
            dummy_long_order(
                dec!(21_000),
                Uuid::new_v4(),
                dec!(200),
                Duration::seconds(0),
            ),
            dummy_long_order(
                dec!(22_000),
                Uuid::new_v4(),
                dec!(400),
                Duration::seconds(0),
            ),
            dummy_long_order(
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
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
        };

        let matched_orders = match_order(&order, all_orders).unwrap();

        assert!(matched_orders.is_none());
    }

    #[tokio::test]
    async fn given_matches_will_notify_all_traders() {
        let trader_key = SecretKey::from_slice(&b"Me noob, don't lose money pleazz"[..]).unwrap();
        let trader_pub_key = trader_key.public_key(SECP256K1);
        let maker_key = SecretKey::from_slice(&b"I am a king trader mate, right!?"[..]).unwrap();
        let maker_pub_key = maker_key.public_key(SECP256K1);
        let trader_order_id = Uuid::new_v4();
        let maker_order_id = Uuid::new_v4();
        let oracle_pk = XOnlyPublicKey::from_str(
            "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0",
        )
        .unwrap();
        let maker_order_price = dec!(20_000);
        let expiry_timestamp = OffsetDateTime::now_utc();
        let matched_orders = MatchParams {
            taker_matches: TraderMatchParams {
                trader_id: trader_pub_key,
                filled_with: FilledWith {
                    order_id: trader_order_id,
                    expiry_timestamp,
                    oracle_pk,
                    matches: vec![Match {
                        order_id: maker_order_id,
                        quantity: dec!(100),
                        pubkey: maker_pub_key,
                        execution_price: maker_order_price,
                    }],
                },
            },
            makers_matches: vec![TraderMatchParams {
                trader_id: maker_pub_key,
                filled_with: FilledWith {
                    order_id: maker_order_id,
                    expiry_timestamp,
                    oracle_pk,
                    matches: vec![Match {
                        order_id: trader_order_id,
                        quantity: dec!(100),
                        pubkey: trader_pub_key,
                        execution_price: maker_order_price,
                    }],
                },
            }],
        };
        let mut traders = HashMap::new();
        let (maker_sender, mut maker_receiver) = mpsc::channel::<OrderbookMsg>(1);
        let (trader_sender, mut trader_receiver) = mpsc::channel::<OrderbookMsg>(1);
        traders.insert(maker_pub_key, maker_sender);
        traders.insert(trader_pub_key, trader_sender);

        notify_traders(matched_orders, &traders).await.unwrap();

        let maker_msg = maker_receiver.recv().await.unwrap();
        let trader_msg = trader_receiver.recv().await.unwrap();

        match maker_msg {
            OrderbookMsg::Match(msg) => {
                assert_eq!(msg.order_id, maker_order_id)
            }
            _ => {
                panic!("Invalid message received")
            }
        }

        match trader_msg {
            OrderbookMsg::Match(msg) => {
                assert_eq!(msg.order_id, trader_order_id)
            }
            _ => {
                panic!("Invalid message received")
            }
        }
    }
}
