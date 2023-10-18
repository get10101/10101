use crate::message::OrderbookMessage;
use crate::notifications::NotificationKind;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use bitcoin::XOnlyPublicKey;
use coordinator_commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use orderbook_commons::FilledWith;
use orderbook_commons::Match;
use orderbook_commons::Message;
use orderbook_commons::NewOrder;
use orderbook_commons::Order;
use orderbook_commons::OrderReason;
use orderbook_commons::OrderState;
use orderbook_commons::OrderType;
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::str::FromStr;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use trade::Direction;
use uuid::Uuid;

/// This value is arbitrarily set to 100 and defines the number of new order messages buffered
/// in the channel.
const NEW_ORDERS_BUFFER_SIZE: usize = 100;
pub struct NewOrderMessage {
    pub new_order: NewOrder,
    pub order_reason: OrderReason,
    pub sender: mpsc::Sender<Result<Order>>,
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
    pub taker_match: TraderMatchParams,
    pub makers_matches: Vec<TraderMatchParams>,
}

impl MatchParams {
    pub fn matches(&self) -> Vec<&TraderMatchParams> {
        let mut matches = vec![&self.taker_match];
        for makers_match in self.makers_matches.iter() {
            matches.push(makers_match);
        }
        matches
    }
}

#[derive(Clone)]
pub struct TraderMatchParams {
    pub trader_id: PublicKey,
    pub filled_with: FilledWith,
}

impl From<&TradeParams> for TraderMatchParams {
    fn from(value: &TradeParams) -> Self {
        TraderMatchParams {
            trader_id: value.pubkey,
            filled_with: value.filled_with.clone(),
        }
    }
}

/// starts the trading task and returns a sender that can be used to send `TradingMessages` to
/// the trading task by spawning a new tokio task that is handling messages
pub fn start(
    pool: Pool<ConnectionManager<PgConnection>>,
    tx_price_feed: broadcast::Sender<Message>,
    notifier: mpsc::Sender<OrderbookMessage>,
    network: Network,
) -> (RemoteHandle<Result<()>>, mpsc::Sender<NewOrderMessage>) {
    let (sender, mut receiver) = mpsc::channel::<NewOrderMessage>(NEW_ORDERS_BUFFER_SIZE);

    let (fut, remote_handle) = async move {
        while let Some(new_order_msg) = receiver.recv().await {
            tokio::spawn({
                let mut conn = pool.get()?;
                let tx_price_feed = tx_price_feed.clone();
                let notifier = notifier.clone();
                async move {
                    let new_order = new_order_msg.new_order;
                    let result = process_new_order(
                        &mut conn,
                        notifier,
                        tx_price_feed,
                        new_order,
                        new_order_msg.order_reason,
                        network,
                    )
                    .await;
                    if let Err(e) = new_order_msg.sender.send(result).await {
                        tracing::error!("Failed to send new order message! Error: {e:#}");
                    }
                }
            });
        }

        Ok(())
    }
    .remote_handle();

    tokio::spawn(fut);

    (remote_handle, sender)
}

/// Processes a new limit and market order
/// TODO(holzeis): The limit and market order models should be separated so we can process the
/// models independently.
///
///
/// Limit order: update price feed
/// Market order: find match and notify traders
async fn process_new_order(
    conn: &mut PgConnection,
    notifier: mpsc::Sender<OrderbookMessage>,
    tx_price_feed: broadcast::Sender<Message>,
    new_order: NewOrder,
    order_reason: OrderReason,
    network: Network,
) -> Result<Order> {
    tracing::info!(trader_id=%new_order.trader_id, "Received a new {:?} order", new_order.order_type);

    if new_order.order_type == OrderType::Limit && new_order.price == Decimal::ZERO {
        return Err(TradingError::InvalidOrder(
            "Limit orders with zero price are not allowed".to_string(),
        ))?;
    }

    // Before processing any match we set all expired limit orders to failed, to ensure the do
    // not get matched.
    // TODO(holzeis): orders should probably do not have an expiry, but should either be
    // replaced or deleted if not wanted anymore.
    let expired_limit_orders = orders::set_expired_limit_orders_to_failed(conn)?;
    for expired_limit_order in expired_limit_orders {
        tx_price_feed
            .send(Message::DeleteOrder(expired_limit_order.id))
            .map_err(|error| anyhow!("Could not update price feed due to '{error}'"))?;
    }

    let order = orders::insert(conn, new_order.clone(), order_reason)
        .map_err(|e| anyhow!("Failed to insert new order into db: {e:#}"))?;

    if new_order.order_type == OrderType::Limit {
        // we only tell everyone about new limit orders
        tx_price_feed
            .send(Message::NewOrder(order.clone()))
            .map_err(|error| anyhow!("Could not update price feed due to '{error}'"))?;
    } else {
        // Reject new order if there is already a matched order waiting for execution.
        if let Some(order) =
            orders::get_by_trader_id_and_state(conn, new_order.trader_id, OrderState::Matched)?
        {
            bail!(TradingError::InvalidOrder(format!(
                    "trader_id={}, order_id={}, Order is currently in execution. Can't accept new orders until the order execution is finished"
                , new_order.trader_id, order.id)));
        }

        let opposite_direction_orders = orders::all_by_direction_and_type(
            conn,
            order.direction.opposite(),
            OrderType::Limit,
            true,
        )?;

        let matched_orders = match match_order(&order, opposite_direction_orders, network) {
            Ok(Some(matched_orders)) => matched_orders,
            Ok(None) => {
                // TODO(holzeis): Currently we still respond to the user immediately if there
                // has been a match or not, that's the reason why we also
                // have to set the order to failed here. But actually we
                // could keep the order until either expired or a
                // match has been found and then update the state correspondingly.

                orders::set_order_state(conn, order.id, OrderState::Failed)?;
                bail!(TradingError::NoMatchFound(format!(
                    "Could not match order {}",
                    order.id
                )));
            }
            Err(e) => {
                orders::set_order_state(conn, order.id, OrderState::Failed)?;
                bail!("Failed to match order. Error {e:#}")
            }
        };

        tracing::info!(trader_id=%order.trader_id, order_id=%order.id, "Found a match with {} makers for new order.", matched_orders.taker_match.filled_with.matches.len());

        let match_params = matched_orders.matches();
        for match_param in match_params {
            matches::insert(conn, match_param)?;

            let trader_id = match_param.trader_id;
            let order_id = match_param.filled_with.order_id.to_string();
            tracing::info!(%trader_id, order_id, "Notifying trader about match");

            let message = match &order.order_reason {
                OrderReason::Manual => Message::Match(match_param.filled_with.clone()),
                OrderReason::Expired => Message::AsyncMatch {
                    order: order.clone(),
                    filled_with: match_param.filled_with.clone(),
                },
            };

            let notification = match &order.order_reason {
                OrderReason::Expired => Some(NotificationKind::PositionExpired),
                OrderReason::Manual => None,
            };
            let msg = OrderbookMessage::TraderMessage {
                trader_id,
                message,
                notification,
            };
            let order_state = match notifier.send(msg).await {
                Ok(()) => {
                    tracing::debug!(%trader_id, order_id, "Successfully notified trader");
                    OrderState::Matched
                }
                Err(e) => {
                    tracing::warn!(%trader_id, order_id, "Failed to send trader message. Error: {e:#}");

                    if order.order_type == OrderType::Limit {
                        // FIXME: The maker is currently not connected to the web socket so we
                        // can't notify him about a trade. However, trades are always accepted
                        // by the maker at the moment so in order to not have all limit orders
                        // in order state `Match` we are setting the order to `Taken` even if we
                        // couldn't notify the maker.

                        OrderState::Taken
                    } else {
                        OrderState::Matched
                    }
                }
            };

            tracing::debug!(
                %trader_id,
                order_id,
                "Updating the order state to {order_state:?}"
            );
            orders::set_order_state(conn, match_param.filled_with.order_id, order_state)?;
        }
    }

    Ok(order)
}

/// Matches a provided market order with limit orders from the DB
///
/// If the order is a long order, we return the short orders sorted by price (highest first)
/// If the order is a short order, we return the long orders sorted by price (lowest first)
///
/// Note: `opposite_direction_orders` should contain only relevant orders. For safety this function
/// will filter it again though
#[autometrics]
fn match_order(
    order: &Order,
    opposite_direction_orders: Vec<Order>,
    network: Network,
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

    let expiry_timestamp =
        coordinator_commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);

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
                            id: Uuid::new_v4(),
                            order_id: order.id,
                            quantity: order.quantity,
                            pubkey: order.trader_id,
                            execution_price: maker_order.price,
                        }],
                    },
                },
                Match {
                    id: Uuid::new_v4(),
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
        taker_match: TraderMatchParams {
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

#[cfg(test)]
pub mod tests {
    use crate::orderbook::trading::match_order;
    use crate::orderbook::trading::sort_orders;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::Network;
    use orderbook_commons::Order;
    use orderbook_commons::OrderReason;
    use orderbook_commons::OrderState;
    use orderbook_commons::OrderType;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::Duration;
    use time::OffsetDateTime;
    use trade::ContractSymbol;
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
            direction: Direction::Long,
            leverage: 1.0,
            contract_symbol: ContractSymbol::BtcUsd,
            quantity,
            order_type: OrderType::Limit,
            timestamp: OffsetDateTime::now_utc() + timestamp_delay,
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
            order_state: OrderState::Open,
            order_reason: OrderReason::Manual,
            stable: false,
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
            direction: Direction::Short,
            leverage: 1.0,
            contract_symbol: ContractSymbol::BtcUsd,
            quantity: dec!(100),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
            order_state: OrderState::Open,
            order_reason: OrderReason::Manual,
            stable: false,
        };

        let matched_orders = match_order(&order, all_orders, Network::Bitcoin)
            .unwrap()
            .unwrap();

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

        assert_eq!(matched_orders.taker_match.filled_with.order_id, order.id);
        assert_eq!(matched_orders.taker_match.filled_with.matches.len(), 1);
        assert_eq!(
            matched_orders
                .taker_match
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
            direction: Direction::Short,
            leverage: 1.0,
            contract_symbol: ContractSymbol::BtcUsd,
            quantity: dec!(200),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
            order_state: OrderState::Open,
            order_reason: OrderReason::Manual,
            stable: false,
        };

        assert!(match_order(&order, all_orders, Network::Bitcoin).is_err());
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
            direction: Direction::Long,
            leverage: 1.0,
            contract_symbol: ContractSymbol::BtcUsd,
            quantity: dec!(200),
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
            order_state: OrderState::Open,
            order_reason: OrderReason::Manual,
            stable: false,
        };

        let matched_orders = match_order(&order, all_orders, Network::Bitcoin).unwrap();

        assert!(matched_orders.is_none());
    }
}
