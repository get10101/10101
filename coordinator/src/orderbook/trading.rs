use crate::db;
use crate::message::OrderbookMessage;
use crate::node::Node;
use crate::notifications::NotificationKind;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::referrals;
use crate::trade::TradeExecutor;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::Amount;
use bitcoin::Network;
use commons::ChannelOpeningParams;
use commons::FilledWith;
use commons::Match;
use commons::Message;
use commons::Message::TradeError;
use commons::Order;
use commons::OrderReason;
use commons::OrderState;
use commons::OrderType;
use commons::TradeAndChannelParams;
use commons::TradeParams;
use commons::TradingError;
use futures::future::RemoteHandle;
use futures::FutureExt;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use std::cmp::Ordering;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

/// This value is arbitrarily set to 100 and defines the number of new order messages buffered in
/// the channel.
const NEW_ORDERS_BUFFER_SIZE: usize = 100;

pub struct NewOrderMessage {
    pub order: Order,
    pub order_reason: OrderReason,
    pub channel_opening_params: Option<ChannelOpeningParams>,
}

#[derive(Clone)]
pub struct MatchParams {
    pub taker_match: TraderMatchParams,
    pub makers_matches: Vec<TraderMatchParams>,
}

#[derive(Clone)]
pub struct TraderMatchParams {
    pub trader_id: PublicKey,
    pub filled_with: FilledWith,
}

/// Spawn a task that processes [`NewOrderMessage`]s.
///
/// To feed messages to this task, the caller can use the corresponding
/// [`mpsc::Sender<NewOrderMessage>`] returned.
pub fn start(
    node: Node,
    tx_orderbook_feed: broadcast::Sender<Message>,
    notifier: mpsc::Sender<OrderbookMessage>,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> (RemoteHandle<()>, mpsc::Sender<NewOrderMessage>) {
    let (sender, mut receiver) = mpsc::channel::<NewOrderMessage>(NEW_ORDERS_BUFFER_SIZE);

    let (fut, remote_handle) = async move {
        while let Some(new_order_msg) = receiver.recv().await {
            tokio::spawn({
                let tx_orderbook_feed = tx_orderbook_feed.clone();
                let notifier = notifier.clone();
                let node = node.clone();
                async move {
                    let new_order = new_order_msg.order;
                    let trader_id = new_order.trader_id;
                    let order_id = new_order.id;
                    let channel_opening_params = new_order_msg.channel_opening_params;

                    tracing::trace!(
                        %trader_id,
                        %order_id,
                        order_type = ?new_order.order_type,
                        "Processing new order",
                    );

                    if let Err(error) = match new_order.order_type {
                        OrderType::Market => {
                            process_new_market_order(
                                node,
                                notifier.clone(),
                                new_order,
                                network,
                                oracle_pk,
                                channel_opening_params
                            )
                            .await
                        }
                        OrderType::Limit => {
                            process_new_limit_order(
                                node,
                                tx_orderbook_feed,
                                new_order,
                            )
                            .await
                        }
                    } {
                        // TODO(holzeis): the maker is currently not subscribed to the websocket
                        // api, hence it wouldn't receive the error message.
                        if let Err(e) = notifier
                            .send(OrderbookMessage::TraderMessage {
                                trader_id,
                                message: TradeError { order_id, error },
                                notification: None,
                            })
                            .await
                        {
                            tracing::error!(%trader_id, %order_id, "Failed to send trade error. Error: {e:#}");
                        }
                    }
                }
            });
        }

        tracing::error!("Channel closed");
    }
    .remote_handle();

    tokio::spawn(fut);

    (remote_handle, sender)
}

pub async fn process_new_limit_order(
    node: Node,
    tx_orderbook_feed: broadcast::Sender<Message>,
    order: Order,
) -> Result<(), TradingError> {
    let mut conn = spawn_blocking(move || node.pool.get())
        .await
        .expect("task to complete")
        .map_err(|e| anyhow!("{e:#}"))?;

    // Before processing any match we set all expired limit orders to failed, to ensure they do not
    // get matched.
    //
    // TODO(holzeis): Orders should probably not have an expiry, but should either be replaced or
    // deleted if not wanted anymore.
    // TODO: I don't think this is necessary anymore. We are manually deleting orders now.
    let expired_limit_orders =
        orders::set_expired_limit_orders_to_expired(&mut conn).map_err(|e| anyhow!("{e:#}"))?;
    for expired_limit_order in expired_limit_orders {
        tx_orderbook_feed
            .send(Message::DeleteOrder(expired_limit_order.id))
            .context("Could not update price feed")?;
    }

    tx_orderbook_feed
        .send(Message::NewOrder(order))
        .map_err(|e| anyhow!(e))
        .context("Could not update price feed")?;

    Ok(())
}

// TODO(holzeis): This functions runs multiple inserts in separate db transactions. This should only
// happen in a single transaction to ensure either all data or nothing is stored to the database.
pub async fn process_new_market_order(
    node: Node,
    notifier: mpsc::Sender<OrderbookMessage>,
    order: Order,
    network: Network,
    oracle_pk: XOnlyPublicKey,
    channel_opening_params: Option<ChannelOpeningParams>,
) -> Result<(), TradingError> {
    let mut conn = spawn_blocking({
        let node = node.clone();
        move || node.pool.get()
    })
    .await
    .expect("task to complete")
    .map_err(|e| anyhow!("{e:#}"))?;

    // Reject new order if there is already a matched order waiting for execution.
    if let Some(order) =
        orders::get_by_trader_id_and_state(&mut conn, order.trader_id, OrderState::Matched)
            .map_err(|e| anyhow!("{e:#}"))?
    {
        return Err(TradingError::InvalidOrder(format!(
            "trader_id={}, order_id={}. Order is currently in execution. \
                 Can't accept new orders until the order execution is finished",
            order.trader_id, order.id
        )));
    }

    let opposite_direction_limit_orders = orders::all_by_direction_and_type(
        &mut conn,
        order.direction.opposite(),
        OrderType::Limit,
        true,
    )
    .map_err(|e| anyhow!("{e:#}"))?;

    let fee_percent = { node.settings.read().await.order_matching_fee_rate };
    let fee_percent = Decimal::try_from(fee_percent).expect("to fit into decimal");

    let trader_pubkey_string = order.trader_id.to_string();
    let status = referrals::get_referral_status(order.trader_id, &mut conn)?;
    let fee_discount = status.referral_fee_bonus;
    let fee_percent = fee_percent - (fee_percent * fee_discount);

    tracing::debug!(
        trader_pubkey = trader_pubkey_string,
        %fee_discount, total_fee_percent = %fee_percent, "Fee discount calculated");

    let matched_orders = match match_order(
        &order,
        opposite_direction_limit_orders,
        network,
        oracle_pk,
        fee_percent,
    ) {
        Ok(Some(matched_orders)) => matched_orders,
        Ok(None) => {
            // TODO(holzeis): Currently we still respond to the user immediately if there
            // has been a match or not, that's the reason why we also have to set the order
            // to failed here. But actually we could keep the order until either expired or
            // a match has been found and then update the state accordingly.

            orders::set_order_state(&mut conn, order.id, OrderState::Failed)
                .map_err(|e| anyhow!("{e:#}"))?;
            return Err(TradingError::NoMatchFound(format!(
                "Could not match order {}",
                order.id
            )));
        }
        Err(e) => {
            orders::set_order_state(&mut conn, order.id, OrderState::Failed)
                .map_err(|e| anyhow!("{e:#}"))?;
            return Err(TradingError::Other(format!("Failed to match order: {e:#}")));
        }
    };

    tracing::info!(
        trader_id=%order.trader_id,
        order_id=%order.id,
        "Found a match with {} makers for new order",
        matched_orders.taker_match.filled_with.matches.len()
    );

    for match_param in matched_orders.matches() {
        matches::insert(&mut conn, match_param)?;

        let trader_id = match_param.trader_id;
        let order_id = match_param.filled_with.order_id.to_string();

        tracing::info!(%trader_id, order_id, "Notifying trader about match");

        let message = match &order.order_reason {
            OrderReason::Manual => Message::Match(match_param.filled_with.clone()),
            OrderReason::Expired
            | OrderReason::TraderLiquidated
            | OrderReason::CoordinatorLiquidated => Message::AsyncMatch {
                order: order.clone(),
                filled_with: match_param.filled_with.clone(),
            },
        };

        let notification = match &order.order_reason {
            OrderReason::Expired => Some(NotificationKind::PositionExpired),
            OrderReason::TraderLiquidated => Some(NotificationKind::Custom {
                title: "Woops, you got liquidated 💸".to_string(),
                message: "Open your app to execute the liquidation".to_string(),
            }),
            OrderReason::CoordinatorLiquidated => Some(NotificationKind::Custom {
                title: "Your counterparty got liquidated 💸".to_string(),
                message: "Open your app to execute the liquidation".to_string(),
            }),
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
                tracing::warn!(%trader_id, order_id, "Failed to send trader message: {e:#}");

                if order.order_type == OrderType::Limit {
                    // FIXME: The maker is currently not connected to the WebSocket so we can't
                    // notify him about a trade. However, trades are always accepted by the
                    // maker at the moment so in order to not have all limit orders in order
                    // state `Match` we are setting the order to `Taken` even if we couldn't
                    // notify the maker.

                    OrderState::Taken
                } else {
                    OrderState::Matched
                }
            }
        };

        tracing::debug!(%trader_id, order_id, "Updating the order state to {order_state:?}");

        orders::set_order_state(&mut conn, match_param.filled_with.order_id, order_state)
            .map_err(|e| anyhow!("{e:#}"))?;
    }

    if let Some(channel_opening_params) = channel_opening_params {
        db::channel_opening_params::insert(&mut conn, order.id, channel_opening_params)
            .map_err(|e| anyhow!("{e:#}"))?;
    }

    if node.inner.is_connected(order.trader_id) {
        tracing::info!(trader_id = %order.trader_id, order_id = %order.id, order_reason = ?order.order_reason, "Executing trade for match");
        let trade_executor = TradeExecutor::new(node.clone(), notifier);

        trade_executor
            .execute(&TradeAndChannelParams {
                trade_params: TradeParams {
                    pubkey: order.trader_id,
                    contract_symbol: ContractSymbol::BtcUsd,
                    leverage: order.leverage,
                    quantity: order.quantity.to_f32().expect("to fit into f32"),
                    direction: order.direction,
                    filled_with: matched_orders.taker_match.filled_with,
                },
                trader_reserve: channel_opening_params.map(|p| p.trader_reserve),
                coordinator_reserve: channel_opening_params.map(|p| p.coordinator_reserve),
            })
            .await;
    } else {
        match order.order_reason {
            OrderReason::Manual => {
                tracing::warn!(trader_id = %order.trader_id, order_id = %order.id, order_reason = ?order.order_reason, "Skipping trade execution as trader is not connected")
            }
            OrderReason::Expired
            | OrderReason::TraderLiquidated
            | OrderReason::CoordinatorLiquidated => {
                tracing::info!(trader_id = %order.trader_id, order_id = %order.id, order_reason = ?order.order_reason, "Skipping trade execution as trader is not connected")
            }
        }
    }

    Ok(())
}

/// Matches an [`Order`] of [`OrderType::Market`] with a list of [`Order`]s of [`OrderType::Limit`].
///
/// The caller is expected to provide a list of `opposite_direction_orders` of [`OrderType::Limit`]
/// and opposite [`Direction`] to the `market_order`. We nevertheless ensure that this is the case
/// to be on the safe side.

fn match_order(
    market_order: &Order,
    opposite_direction_orders: Vec<Order>,
    network: Network,
    oracle_pk: XOnlyPublicKey,
    fee_percent: Decimal,
) -> Result<Option<MatchParams>> {
    if market_order.order_type == OrderType::Limit {
        // We don't match limit orders with other limit orders at the moment.
        return Ok(None);
    }

    let opposite_direction_orders = opposite_direction_orders
        .into_iter()
        .filter(|o| !o.direction.eq(&market_order.direction))
        .collect();

    let mut orders = sort_orders(opposite_direction_orders, market_order.direction);

    let mut remaining_quantity = market_order.quantity;
    let mut matched_orders = vec![];
    while !orders.is_empty() {
        let matched_order = orders.remove(0);
        remaining_quantity -= matched_order.quantity;
        matched_orders.push(matched_order);

        if remaining_quantity <= Decimal::ZERO {
            break;
        }
    }

    // For the time being we do not want to support multi-matches.
    if matched_orders.len() > 1 {
        bail!("More than one matched order, please reduce order quantity");
    }

    if matched_orders.is_empty() {
        return Ok(None);
    }

    let expiry_timestamp = commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);

    let matches = matched_orders
        .iter()
        .map(|maker_order| {
            let matching_fee = market_order.quantity / maker_order.price * fee_percent;
            let matching_fee = matching_fee.round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);
            let matching_fee = match Amount::from_btc(matching_fee.to_f64().expect("to fit")) {
                Ok(fee) => {fee}
                Err(err) => {
                    tracing::error!(
                        trader_pubkey = maker_order.trader_id.to_string(),
                        order_id = maker_order.id.to_string(),
                        "Failed calculating order matching fee for order {err:?}. Falling back to 0");
                    Amount::ZERO
                }
            };
            (
                TraderMatchParams {
                    trader_id: maker_order.trader_id,
                    filled_with: FilledWith {
                        order_id: maker_order.id,
                        expiry_timestamp,
                        oracle_pk,
                        matches: vec![Match {
                            id: Uuid::new_v4(),
                            order_id: market_order.id,
                            quantity: market_order.quantity,
                            pubkey: market_order.trader_id,
                            execution_price: maker_order.price,
                            matching_fee,
                        }],
                    },
                },
                Match {
                    id: Uuid::new_v4(),
                    order_id: maker_order.id,
                    quantity: market_order.quantity,
                    pubkey: maker_order.trader_id,
                    execution_price: maker_order.price,
                    matching_fee,
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
            trader_id: market_order.trader_id,
            filled_with: FilledWith {
                order_id: market_order.id,
                expiry_timestamp,
                oracle_pk,
                matches: taker_matches,
            },
        },
        makers_matches: maker_matches,
    }))
}

/// Sort the provided list of limit [`Order`]s based on the [`Direction`] of the market order to be
/// matched.
///
/// For matching a market order and limit orders we have to
///
/// - take the highest rate if the market order is short; and
///
/// - take the lowest rate if the market order is long.
///
/// Hence, the orders are sorted accordingly:
///
/// - If the market order is short, the limit orders are sorted in descending order of
/// price.
///
/// - If the market order is long, the limit orders are sorted in ascending order of price.
///
/// Additionally, if two orders have the same price, the one with the earlier `timestamp` takes
/// precedence.
fn sort_orders(mut limit_orders: Vec<Order>, market_order_direction: Direction) -> Vec<Order> {
    limit_orders.sort_by(|a, b| {
        if a.price.cmp(&b.price) == Ordering::Equal {
            return a.timestamp.cmp(&b.timestamp);
        }

        match market_order_direction {
            // Ascending order.
            Direction::Long => a.price.cmp(&b.price),
            // Descending order.
            Direction::Short => b.price.cmp(&a.price),
        }
    });

    limit_orders
}

impl MatchParams {
    fn matches(&self) -> Vec<&TraderMatchParams> {
        std::iter::once(&self.taker_match)
            .chain(self.makers_matches.iter())
            .collect()
    }
}

impl From<&TradeParams> for TraderMatchParams {
    fn from(value: &TradeParams) -> Self {
        TraderMatchParams {
            trader_id: value.pubkey,
            filled_with: value.filled_with.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::Duration;
    use trade::ContractSymbol;

    #[test]
    fn when_short_then_sort_desc() {
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

        let orders = sort_orders(orders, Direction::Short);
        assert_eq!(orders[0], order2);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order1);
    }

    #[test]
    fn when_long_then_sort_asc() {
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

        let orders = sort_orders(orders, Direction::Long);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order3);
        assert_eq!(orders[2], order2);
    }

    #[test]
    fn when_all_same_price_sort_by_id() {
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

        let orders = sort_orders(orders, Direction::Long);
        assert_eq!(orders[0], order1);
        assert_eq!(orders[1], order2);
        assert_eq!(orders[2], order3);

        let orders = sort_orders(orders, Direction::Short);
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

        let matched_orders = match_order(
            &order,
            all_orders,
            Network::Bitcoin,
            get_oracle_public_key(),
            Decimal::ZERO,
        )
        .unwrap()
        .unwrap();

        assert_eq!(matched_orders.makers_matches.len(), 1);
        let maker_matches = matched_orders
            .makers_matches
            .first()
            .unwrap()
            .filled_with
            .matches
            .clone();
        assert_eq!(maker_matches.len(), 1);
        assert_eq!(maker_matches.first().unwrap().quantity, dec!(100));

        assert_eq!(matched_orders.taker_match.filled_with.order_id, order.id);
        assert_eq!(matched_orders.taker_match.filled_with.matches.len(), 1);
        assert_eq!(
            matched_orders
                .taker_match
                .filled_with
                .matches
                .first()
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

        assert!(match_order(
            &order,
            all_orders,
            Network::Bitcoin,
            get_oracle_public_key(),
            Decimal::ZERO,
        )
        .is_err());
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

        let matched_orders = match_order(
            &order,
            all_orders,
            Network::Bitcoin,
            get_oracle_public_key(),
            Decimal::ZERO,
        )
        .unwrap();

        assert!(matched_orders.is_none());
    }

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

    fn get_oracle_public_key() -> XOnlyPublicKey {
        XOnlyPublicKey::from_str("16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0")
            .unwrap()
    }
}
