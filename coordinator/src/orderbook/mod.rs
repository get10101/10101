use crate::message::TraderMessage;
use crate::message::TraderSender;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::orderbook::match_order::MatchExecutorSender;
use crate::orderbook::match_order::MatchedOrder;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::vec;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use uuid::Uuid;
use xxi_node::commons;
use xxi_node::commons::Message;
use xxi_node::commons::Message::DeleteOrder;
use xxi_node::commons::NewLimitOrder;
use xxi_node::commons::NewMarketOrder;
use xxi_node::commons::Order;
use xxi_node::commons::OrderReason;
use xxi_node::commons::OrderState;
use xxi_node::commons::OrderType;
use xxi_node::commons::TradingError;

pub mod collaborative_revert;
pub mod db;
pub mod match_order;
pub mod trading;
pub mod websocket;

#[cfg(test)]
mod tests;

struct OrderbookActionSender {
    // implementing a sync sender to ensure the orderbook action has been sent before continuing.
    orderbook_executor: std::sync::mpsc::Sender<OrderbookAction>,
}

impl OrderbookActionSender {
    pub fn send(&self, action: OrderbookAction) {
        if let Err(e) = self.orderbook_executor.send(action) {
            tracing::error!("Failed to send orderbook action. Error: {e:#}");
        }
    }
}

#[derive(Debug)]
pub enum OrderbookAction {
    AddLimitOrder(NewLimitOrder),
    AddMarketOrder {
        new_order: NewMarketOrder,
        order_reason: OrderReason,
    },
    FailOrder {
        order_id: Uuid,
        order_reason: OrderReason,
    },
    FillOrder {
        order_id: Uuid,
        quantity: Decimal,
    },
    RemoveOrder(Uuid),
}

fn spawn_orderbook_executor(
    pool: Pool<ConnectionManager<PgConnection>>,
) -> std::sync::mpsc::Sender<OrderbookAction> {
    let (sender, receiver) = std::sync::mpsc::channel::<OrderbookAction>();

    tokio::spawn({
        async move {
            while let Ok(action) = receiver.recv() {
                tracing::trace!(?action, "Processing orderbook action.");
                if let Err(e) = match action {
                    OrderbookAction::AddLimitOrder(new_order) => spawn_blocking({
                        let pool = pool.clone();
                        move || {
                            let mut conn = pool.clone().get()?;
                            orders::insert_limit_order(&mut conn, new_order, OrderReason::Manual)
                                .map_err(|e| anyhow!(e))
                                .context("Failed to insert new order into DB")?;
                            anyhow::Ok(())
                        }
                    }),
                    OrderbookAction::RemoveOrder(order_id) => {
                        spawn_blocking({
                            let pool = pool.clone();
                            move || {
                                let mut conn = pool.clone().get()?;
                                let matches =
                                    matches::get_matches_by_order_id(&mut conn, order_id)?;

                                if !matches.is_empty() {
                                    // order has been at least partially matched.
                                    let matched_quantity = matches.iter().map(|m| m.quantity).sum();

                                    orders::set_order_state_partially_taken(
                                        &mut conn,
                                        order_id,
                                        matched_quantity,
                                    )?;
                                } else {
                                    orders::delete(&mut conn, order_id)?;
                                }

                                anyhow::Ok(())
                            }
                        })
                    }
                    OrderbookAction::AddMarketOrder {
                        new_order,
                        order_reason,
                    } => spawn_blocking({
                        let pool = pool.clone();
                        move || {
                            let mut conn = pool.clone().get()?;
                            orders::insert_market_order(&mut conn, new_order, order_reason)
                                .map_err(|e| anyhow!(e))
                                .context("Failed to insert new order into DB")?;

                            anyhow::Ok(())
                        }
                    }),
                    OrderbookAction::FailOrder {
                        order_id,
                        order_reason,
                    } => spawn_blocking({
                        let pool = pool.clone();
                        move || {
                            let mut conn = pool.get()?;
                            let order_state = match order_reason {
                                OrderReason::CoordinatorLiquidated
                                | OrderReason::TraderLiquidated
                                | OrderReason::Manual => OrderState::Failed,
                                OrderReason::Expired => OrderState::Expired,
                            };
                            orders::set_order_state(&mut conn, order_id, order_state)
                                .context("Failed to set order state")?;

                            anyhow::Ok(())
                        }
                    }),
                    OrderbookAction::FillOrder { order_id, quantity } => spawn_blocking({
                        let pool = pool.clone();
                        move || {
                            let mut conn = pool.get()?;
                            let order = orders::get_with_id(&mut conn, order_id)?
                                .context("Missing order.")?;

                            match order.quantity.cmp(&quantity) {
                                // The order quantity is greater than the consumed quantity. The
                                // order remains open, but we reduce the quantity.
                                Ordering::Greater => {
                                    orders::update_quantity(
                                        &mut conn,
                                        order_id,
                                        order.quantity - quantity,
                                    )?;
                                }
                                // The order has been matched to its full quantity. The order is set
                                // to taken.
                                Ordering::Equal => {
                                    orders::set_order_state(
                                        &mut conn,
                                        order_id,
                                        OrderState::Taken,
                                    )?;
                                }
                                Ordering::Less => debug_assert!(
                                    false,
                                    "Can't take more quantity than the volume of the order."
                                ),
                            }

                            anyhow::Ok(())
                        }
                    }),
                }
                .await
                {
                    tracing::error!(?action, "Failed to process orderbook action. Error: {e:#}");
                }
            }

            tracing::warn!("Sender closed the channel. Stop listening to orderbook actions.");
        }
    });

    sender
}

struct Orderbook {
    shorts: OrderbookSide,
    longs: OrderbookSide,
    orderbook_action_sender: OrderbookActionSender,
    trader_sender: TraderSender,
    match_executor: MatchExecutorSender,
    notifier: mpsc::Sender<Notification>,
}

struct OrderbookSide {
    orders: BTreeMap<Decimal, Vec<Order>>,
    direction: commons::Direction,
}

impl OrderbookSide {
    pub fn new(orders: Vec<Order>, direction: commons::Direction) -> Self {
        let mut orders_map: BTreeMap<Decimal, Vec<Order>> = BTreeMap::new();
        for order in orders {
            orders_map.entry(order.price).or_default().push(order);
        }
        for vec in orders_map.values_mut() {
            vec.sort_by_key(|order| order.timestamp);
        }
        OrderbookSide {
            orders: orders_map,
            direction,
        }
    }
    /// adds the given order to the orderbook.
    pub fn add_order(&mut self, order: Order) {
        let entry = self.orders.entry(order.price).or_default();
        entry.push(order);
    }

    /// removes the order by the given id from the orderbook. Returns true if the order was found,
    /// returns false otherwise.
    fn remove_order(&mut self, order_id: Uuid) -> bool {
        for (_price, orders) in self.orders.iter_mut() {
            if let Some(pos) = orders.iter().position(|order| order.id == order_id) {
                orders.remove(pos);
                if orders.is_empty() {
                    self.orders.retain(|_price, orders| !orders.is_empty());
                }
                return true;
            }
        }
        false
    }

    /// returns the a sorted vec orders of the orderbook side with the best price at first.
    fn get_orders(&self) -> Vec<Order> {
        let mut sorted_orders: Vec<Order> = self
            .orders
            .values()
            .flat_map(|orders| orders.iter().copied())
            .collect();

        if commons::Direction::Short == self.direction {
            sorted_orders.reverse();
        }

        sorted_orders
    }

    /// matches orders from the orderbook for the given quantity.
    pub fn match_order(&self, quantity: Decimal) -> Vec<Order> {
        let mut matched_orders = vec![];

        let mut quantity = quantity;
        for order in self.get_orders().iter() {
            if order.is_expired() {
                // ignore expired orders.
                continue;
            }

            match order.quantity.cmp(&quantity) {
                Ordering::Less => {
                    // if the found order has less quantity we subtract the full quantity from the
                    // searched quantity and add the limit order to the matched orders.
                    quantity -= order.quantity;
                    matched_orders.push(*order);
                }
                Ordering::Greater => {
                    // we found enough liquidity in the order book to match the order.
                    matched_orders.push(Order { quantity, ..*order });
                    break;
                }
                Ordering::Equal => {
                    // we found a perfect match for the searched for quantity.
                    matched_orders.push(*order);
                    break;
                }
            }
        }

        matched_orders
    }

    /// commits the given matched orders to the in memory orderbook, by removing the volume from the
    /// orderbook. If an order is partially matched the quantity of the matched order is reduced.s
    pub fn commit_matched_orders(&mut self, matched_orders: &Vec<Order>) {
        for order in matched_orders {
            if let Some(vec) = self.orders.get_mut(&order.price) {
                if let Some(existing_order) = vec.iter_mut().find(|o| o.id == order.id) {
                    match existing_order.quantity.cmp(&order.quantity) {
                        Ordering::Equal => {
                            // If the quantities are equal, remove the order
                            vec.retain(|o| o.id != order.id);
                        }
                        Ordering::Greater => {
                            // If the existing order quantity is greater, update the quantity
                            existing_order.quantity -= order.quantity;
                        }
                        Ordering::Less => debug_assert!(
                            false,
                            "matched quantity is bigger than the existing order"
                        ),
                    }
                }
                if vec.is_empty() {
                    self.orders.remove(&order.price);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct OrderMatchingFeeRate {
    pub maker: Decimal,
    pub taker: Decimal,
}

impl Orderbook {
    /// Initializes the orderbook with non expired open limit orders.
    async fn new(
        pool: Pool<ConnectionManager<PgConnection>>,
        notifier: mpsc::Sender<Notification>,
        match_executor: mpsc::Sender<MatchedOrder>,
        trader_sender: mpsc::Sender<TraderMessage>,
    ) -> Result<Self> {
        let all_orders = spawn_blocking({
            let mut conn = pool.clone().get()?;
            move || {
                // TODO(holzeis): join with trades to get partially matched orders.
                let all_orders =
                    orders::get_all_orders(&mut conn, OrderType::Limit, OrderState::Open, true)?;
                anyhow::Ok(all_orders)
            }
        })
        .await??;

        let shorts = all_orders
            .clone()
            .into_iter()
            .filter(|o| o.direction == commons::Direction::Short)
            .collect::<Vec<_>>();

        let longs = all_orders
            .into_iter()
            .filter(|o| o.direction == commons::Direction::Long)
            .collect::<Vec<_>>();

        let orderbook_executor = spawn_orderbook_executor(pool);

        let orderbook = Self {
            shorts: OrderbookSide::new(shorts, commons::Direction::Short),
            longs: OrderbookSide::new(longs, commons::Direction::Long),
            orderbook_action_sender: OrderbookActionSender { orderbook_executor },
            trader_sender: TraderSender {
                sender: trader_sender,
            },
            match_executor: MatchExecutorSender {
                sender: match_executor,
            },
            notifier,
        };

        Ok(orderbook)
    }

    /// Adds a limit order to the orderbook.
    fn add_limit_order(&mut self, new_order: NewLimitOrder) -> Message {
        self.orderbook_action_sender
            .send(OrderbookAction::AddLimitOrder(new_order));

        let order: Order = new_order.into();
        match order.direction {
            commons::Direction::Short => self.shorts.add_order(order),
            commons::Direction::Long => self.longs.add_order(order),
        }
        Message::NewOrder(order)
    }

    /// Matches a market order against the orderbook. Will fail if the market order can't be fully
    /// matched.
    fn match_market_order(&mut self, new_order: NewMarketOrder, order_reason: OrderReason) {
        self.orderbook_action_sender
            .send(OrderbookAction::AddMarketOrder {
                new_order,
                order_reason,
            });

        let order = Order {
            order_reason,
            ..new_order.into()
        };

        // find a match for the market order.
        let matched_orders = match order.direction.opposite() {
            commons::Direction::Short => self.shorts.match_order(order.quantity),
            commons::Direction::Long => self.longs.match_order(order.quantity),
        };

        let order_id = order.id;
        let trader_pubkey = order.trader_id;

        let matched_quantity: Decimal = matched_orders.iter().map(|o| o.quantity).sum();
        if matched_quantity != order.quantity {
            // not enough liquidity in the orderbook.
            tracing::warn!(
                trader_pubkey = %order.trader_id,
                order_id = %order.id,
                wanted = %order.quantity,
                got = %matched_quantity,
                "Couldn't match order due to insufficient liquidity in the orderbook."
            );

            self.orderbook_action_sender
                .send(OrderbookAction::FailOrder {
                    order_id: order.id,
                    order_reason,
                });

            let message = TraderMessage {
                trader_id: trader_pubkey,
                message: Message::TradeError {
                    order_id,
                    error: TradingError::NoMatchFound(order.id.to_string()),
                },
                notification: None,
            };
            self.trader_sender.send(message);
        } else {
            // apply changes to the in memory orderbook.
            match order.direction.opposite() {
                commons::Direction::Short => self.shorts.commit_matched_orders(&matched_orders),
                commons::Direction::Long => self.longs.commit_matched_orders(&matched_orders),
            }

            let order = Order {
                order_state: OrderState::Taken,
                ..order
            };

            self.orderbook_action_sender
                .send(OrderbookAction::FillOrder {
                    order_id,
                    quantity: order.quantity,
                });

            for order in &matched_orders {
                let order = Order {
                    order_state: OrderState::Taken,
                    ..*order
                };
                self.orderbook_action_sender
                    .send(OrderbookAction::FillOrder {
                        order_id: order.id,
                        quantity: order.quantity,
                    });
            }

            // execute matched order
            self.match_executor.send(MatchedOrder {
                order,
                matched: matched_orders,
            });

            notify_user(self.notifier.clone(), trader_pubkey, order_reason);
        }
    }

    /// Updates a limit order in the orderbook by removing it and adding it anew.
    fn update_limit_order(&mut self, order: Order) -> Message {
        self.remove_order(order.id);
        let new_order = NewLimitOrder {
            id: order.id,
            contract_symbol: order.contract_symbol,
            price: order.price,
            quantity: order.quantity,
            trader_id: order.trader_id,
            direction: order.direction,
            leverage: Decimal::try_from(order.leverage).expect("to fit"),
            expiry: order.expiry,
            stable: false,
        };
        self.add_limit_order(new_order);

        Message::Update(order)
    }

    /// Removes a limit order from the orderbook. Ignores removing market orders, since they aren't
    /// stored into the orderbook.
    fn remove_order(&mut self, order_id: Uuid) -> Option<Message> {
        self.orderbook_action_sender
            .send(OrderbookAction::RemoveOrder(order_id));

        if self.shorts.remove_order(order_id) {
            return Some(DeleteOrder(order_id));
        }

        if self.longs.remove_order(order_id) {
            return Some(DeleteOrder(order_id));
        }

        None
    }
}

/// Sends a push notification to the user in case of an expiry or liquidation.
fn notify_user(
    notifier: mpsc::Sender<Notification>,
    trader_pubkey: PublicKey,
    order_reason: OrderReason,
) {
    tokio::spawn({
        async move {
            let notification = match order_reason {
                OrderReason::Expired => Some(NotificationKind::PositionExpired),
                OrderReason::TraderLiquidated => Some(NotificationKind::Custom {
                    title: "Oops, you got liquidated 💸".to_string(),
                    message: "Open your app to execute the liquidation".to_string(),
                }),
                OrderReason::CoordinatorLiquidated => Some(NotificationKind::Custom {
                    title: "Your counterparty got liquidated 💸".to_string(),
                    message: "Open your app to execute the liquidation".to_string(),
                }),
                OrderReason::Manual => None,
            };

            if let Some(notification) = notification {
                tracing::info!(%trader_pubkey, ?order_reason, "Notifying trader about match");
                // send user a push notification
                if let Err(e) = notifier
                    .send(Notification::new(trader_pubkey, notification))
                    .await
                {
                    tracing::error!("Failed to notify trader about match. Error: {e:#}")
                }
            }
        }
    });
}

#[cfg(test)]
mod orderbook_tests {
    use crate::orderbook::OrderbookSide;
    use bitcoin::secp256k1::PublicKey;
    use rust_decimal::Decimal;
    use rust_decimal::RoundingStrategy;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::Duration;
    use time::OffsetDateTime;
    use uuid::Uuid;
    use xxi_node::commons::ContractSymbol;
    use xxi_node::commons::Direction;
    use xxi_node::commons::Order;
    use xxi_node::commons::OrderReason;
    use xxi_node::commons::OrderState;
    use xxi_node::commons::OrderType;

    #[test]
    pub fn test_add_order_to_orderbook_side() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order = dummy_order(dec!(100), dec!(50000), Direction::Long);
        longs.add_order(long_order);

        let order = longs.get_orders().first().cloned();

        assert_eq!(Some(long_order), order);
    }

    #[test]
    pub fn test_remove_order_from_orderbook_side() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order = dummy_order(dec!(100), dec!(50000), Direction::Long);
        longs.add_order(long_order);

        longs.remove_order(long_order.id);

        let order = longs.get_orders().first().cloned();
        assert_eq!(None, order);
    }

    #[test]
    pub fn test_remove_invalid_order_id_from_orderbook_side() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order = dummy_order(dec!(100), dec!(50000), Direction::Long);
        longs.add_order(long_order);

        longs.remove_order(Uuid::new_v4());

        let order = longs.get_orders().first().cloned();
        assert_eq!(Some(long_order), order);
    }

    #[test]
    pub fn test_match_order_exact_match_single_order() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order = dummy_order(dec!(100), dec!(50000), Direction::Long);
        longs.add_order(long_order);

        let matched_orders = longs.match_order(dec!(100));
        assert_eq!(1, matched_orders.len());
        assert_eq!(dec!(100), matched_orders.iter().map(|m| m.quantity).sum());

        longs.commit_matched_orders(&matched_orders);

        assert_eq!(None, longs.get_orders().first());
    }

    #[test]
    pub fn test_match_order_partial_limit_order_match_single_order() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order = dummy_order(dec!(100), dec!(50000), Direction::Long);
        longs.add_order(long_order);

        let matched_orders = longs.match_order(dec!(25));
        assert_eq!(1, matched_orders.len());
        assert_eq!(dec!(25), matched_orders.iter().map(|m| m.quantity).sum());

        longs.commit_matched_orders(&matched_orders);

        assert_eq!(
            Some(Order {
                quantity: dec!(75),
                ..long_order
            }),
            longs.get_orders().first().cloned()
        );
    }

    #[test]
    pub fn test_match_order_partial_market_order_match_single_order() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order = dummy_order(dec!(100), dec!(50000), Direction::Long);
        longs.add_order(long_order);

        let matched_orders = longs.match_order(dec!(125));
        assert_eq!(1, matched_orders.len());
        assert_eq!(dec!(100), matched_orders.iter().map(|m| m.quantity).sum());

        assert_eq!(Some(long_order), longs.get_orders().first().cloned());
    }

    #[test]
    pub fn test_match_order_partial_match_multiple_orders() {
        let mut longs = OrderbookSide::new(vec![], Direction::Long);

        let long_order_1 = dummy_order(dec!(25), dec!(50100), Direction::Long);
        longs.add_order(long_order_1);

        let long_order_2 = dummy_order(dec!(40), dec!(50200), Direction::Long);
        longs.add_order(long_order_2);

        let long_order_3 = dummy_order(dec!(35), dec!(50300), Direction::Long);
        longs.add_order(long_order_3);

        let matched_orders = longs.match_order(dec!(50));
        assert_eq!(2, matched_orders.len());
        assert_eq!(dec!(50), matched_orders.iter().map(|m| m.quantity).sum());

        assert_eq!(dec!(50149.95), average_entry_price(&matched_orders));

        longs.commit_matched_orders(&matched_orders);

        assert_eq!(
            Some(Order {
                quantity: dec!(15),
                ..long_order_2
            }),
            longs.get_orders().first().cloned()
        );

        assert_eq!(
            Some(Order {
                quantity: dec!(35),
                ..long_order_3
            }),
            longs.get_orders().get(1).cloned()
        );
    }

    fn dummy_order(quantity: Decimal, price: Decimal, direction: Direction) -> Order {
        Order {
            id: Uuid::new_v4(),
            price,
            trader_id: dummy_public_key(),
            direction,
            leverage: 1.0,
            contract_symbol: ContractSymbol::BtcUsd,
            quantity,
            order_type: OrderType::Market,
            timestamp: OffsetDateTime::now_utc(),
            expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
            order_state: OrderState::Open,
            order_reason: OrderReason::Manual,
            stable: false,
        }
    }

    fn dummy_public_key() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .unwrap()
    }

    fn average_entry_price(orders: &[Order]) -> Decimal {
        if orders.is_empty() {
            return Decimal::ZERO;
        }
        if orders.len() == 1 {
            return orders.first().expect("to be exactly one").price;
        }
        let sum_quantity = orders.iter().fold(dec!(0), |acc, m| acc + m.quantity);

        let nominal_prices = orders
            .iter()
            .fold(dec!(0), |acc, m| acc + (m.quantity / m.price));

        (sum_quantity / nominal_prices)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
    }
}
