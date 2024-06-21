use crate::check_version::check_version;
use crate::message::TraderMessage;
use crate::message::TraderSender;
use crate::node::Node;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::orderbook::OrderMatchingFeeRate;
use crate::referrals;
use crate::trade::ExecutableMatch;
use crate::trade::TradeExecutor;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::result::Error::RollbackTransaction;
use diesel::Connection;
use diesel::PgConnection;
use futures::stream::StreamExt;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::RoundingStrategy;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::ReceiverStream;
use xxi_node::commons::MatchState;
use xxi_node::commons::Message::TradeError;
use xxi_node::commons::Order;
use xxi_node::commons::OrderReason;
use xxi_node::node::event::NodeEvent;

enum MatchExecutorEvent {
    MatchedOrder(MatchedOrder),
    NodeEvent(NodeEvent),
}

pub fn spawn_match_executor(
    node: Node,
    node_event_receiver: broadcast::Receiver<NodeEvent>,
    order_matching_fee_rate: OrderMatchingFeeRate,
    trader_sender: TraderSender,
) -> mpsc::Sender<MatchedOrder> {
    let (sender, receiver) = mpsc::channel::<MatchedOrder>(100);

    tokio::spawn({
        let pool = node.pool.clone();
        let trade_executor = TradeExecutor::new(node.clone());
        let trader_sender = trader_sender.clone();

        async move {
            let match_executor = MatchExecutor {
                pool: pool.clone(),
                order_matching_fee_rate,
                trade_executor,
            };
            let receiver_stream =
                ReceiverStream::new(receiver).map(MatchExecutorEvent::MatchedOrder);
            let node_event_stream =
                BroadcastStream::new(node_event_receiver).filter_map(|event| async {
                    match event {
                        Ok(event) => Some(MatchExecutorEvent::NodeEvent(event)),
                        Err(BroadcastStreamRecvError::Lagged(skip)) => {
                            tracing::warn!(%skip, "Lagging behind on node events.");
                            None
                        }
                    }
                });

            let mut merged_stream =
                futures::stream::select(Box::pin(receiver_stream), Box::pin(node_event_stream));

            while let Some(event) = merged_stream.next().await {
                match event {
                    MatchExecutorEvent::MatchedOrder(matched_order) => {
                        let trader = matched_order.order.trader_id;
                        let order_id = matched_order.order.id;
                        let order_reason = matched_order.order.order_reason;
                        tracing::info!(%trader, %order_id, "Processing matched order.");
                        let match_executor = match_executor.clone();
                        let trader_sender = trader_sender.clone();
                        match match_executor.process_matched_order(matched_order).await {
                            Ok(matches) => {
                                for executeable_match in matches {
                                    if let Err(e) = match_executor
                                        .execute_match(executeable_match, order_reason)
                                        .await
                                    {
                                        tracing::error!(%trader, %order_id, "Failed to execute match. Error: {e:#}");

                                        trader_sender.send(TraderMessage {
                                            trader_id: trader,
                                            message: TradeError {
                                                order_id,
                                                error: e.into(),
                                            },
                                            notification: None,
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(%trader, %order_id, "Failed to process matched order. Error: {e:#}");

                                trader_sender.send(TraderMessage {
                                    trader_id: trader,
                                    message: TradeError {
                                        order_id,
                                        error: e.into(),
                                    },
                                    notification: None,
                                });
                            }
                        }
                    }
                    MatchExecutorEvent::NodeEvent(NodeEvent::Connected { peer: trader }) => {
                        tracing::info!(%trader, "Checking if user has a pending match.");
                        match match_executor.process_pending_match(trader).await {
                            Ok(Some(executeable_match)) => {
                                let trader = executeable_match.order.trader_id;
                                let order_id = executeable_match.order.id;
                                let order_reason = executeable_match.order.order_reason;
                                tracing::info!(%trader, "Found pending matches.");
                                if let Err(e) = match_executor
                                    .execute_match(executeable_match, order_reason)
                                    .await
                                {
                                    tracing::error!(%trader, %order_id, "Failed to execute match. Error: {e:#}");

                                    trader_sender.send(TraderMessage {
                                        trader_id: trader,
                                        message: TradeError {
                                            order_id,
                                            error: e.into(),
                                        },
                                        notification: None,
                                    });
                                }
                            }
                            Ok(None) => {
                                tracing::debug!(%trader, "No pending matches found.");
                            }
                            Err(e)
                                if e.to_string()
                                    .contains("Please upgrade to the latest version") =>
                            {
                                tracing::info!(%trader, "User is not on the latest version. Skipping check if user needs to be informed about pending matches.");
                            }
                            Err(e) => {
                                tracing::error!(%trader, "Failed to process pending match. Error: {e:#}");
                            }
                        }
                    }
                    MatchExecutorEvent::NodeEvent(_) => {} // ignore other node events.
                }
            }
        }
    });

    sender
}

#[derive(Clone)]
struct MatchExecutor {
    pool: Pool<ConnectionManager<PgConnection>>,
    order_matching_fee_rate: OrderMatchingFeeRate,
    trade_executor: TradeExecutor,
}

impl MatchExecutor {
    async fn process_matched_order(
        &self,
        matched_order: MatchedOrder,
    ) -> Result<Vec<ExecutableMatch>> {
        let matches = spawn_blocking({
            let matched_orders = matched_order.matched.clone();
            let fee_percent = self.order_matching_fee_rate.taker;
            let order = matched_order.order;
            let pool = self.pool.clone();
            let trader = matched_order.order.trader_id;
            move || {
                let mut conn = pool.clone().get()?;
                let mut matches: HashMap<PublicKey, ExecutableMatch> = HashMap::new();
                conn.transaction(|conn| {
                    let status =
                        referrals::get_referral_status(trader, conn).map_err(|_| RollbackTransaction)?;
                    let fee_discount = status.referral_fee_bonus;
                    let fee_percent = fee_percent - (fee_percent * fee_discount);

                    tracing::debug!(%trader, %fee_discount, total_fee_percent = %fee_percent, "Fee discount calculated");

                    for matched_order in matched_orders {
                        let matching_fee = matched_order.quantity / matched_order.price * fee_percent;
                        let matching_fee = matching_fee.round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);
                        let matching_fee = match Amount::from_btc(matching_fee.to_f64().expect("to fit")) {
                            Ok(fee) => fee,
                            Err(err) => {
                                tracing::error!(
                                                trader_pubkey = matched_order.trader_id.to_string(),
                                                order_id = matched_order.id.to_string(),
                                                "Failed calculating order matching fee for order {err:?}. Falling back to 0"
                                            );
                                Amount::ZERO
                            }
                        };

                        let taker_match = matches::insert(conn, &order, &matched_order, matching_fee, MatchState::Pending, matched_order.quantity)?;
                        if let Some(taker_matches) = matches.get_mut(&trader) {
                            taker_matches.matches.push(taker_match);
                        } else {
                            matches.insert(trader, ExecutableMatch {
                                order,
                                matches: vec![taker_match],
                            });
                        }

                        // TODO(holzeis): For now we don't execute the limit order with the maker as the our maker does not
                        // have a dlc channel with the coordinator, hence we set the match directly to filled. Once we
                        // introduce actual makers these matches need to execute them.
                        let _maker_match = matches::insert(conn, &matched_order, &order, matching_fee, MatchState::Filled, matched_order.quantity)?;

                        // TODO(holzeis): Add executable match once we support actual limit orders.
                        // if let Some(maker_matches) = matches.get_mut(&matched_order.trader_id) {
                        //     maker_matches.matches.push(taker_match);
                        // } else {
                        //     matches.insert(order.trader_id, ExecutableMatch{
                        //         order,
                        //         matches: vec![taker_match]
                        //     });
                        // }
                    }

                    diesel::result::QueryResult::Ok(())
                })?;

                anyhow::Ok(matches)
            }
        }).await??;

        Ok(matches.into_values().collect::<Vec<ExecutableMatch>>())
    }

    async fn execute_match(
        &self,
        executable_match: ExecutableMatch,
        order_reason: OrderReason,
    ) -> Result<()> {
        let trader = executable_match.order.trader_id;
        let order_id = executable_match.order.id;
        if self.trade_executor.is_connected(trader) {
            tracing::info!(%trader, %order_id, "Executing pending match");
            match self.trade_executor.execute(executable_match).await {
                Ok(()) => {
                    tracing::info!(%trader, %order_id, ?order_reason, "Successfully proposed trade.");

                    // TODO(holzeis): We should only set the match to filled once the dlc
                    // protocol has finished.
                    if let Err(e) = spawn_blocking({
                        let pool = self.pool.clone();
                        move || {
                            let mut conn = pool.get()?;
                            matches::set_match_state(&mut conn, order_id, MatchState::Filled)?;
                            anyhow::Ok(())
                        }
                    })
                    .await?
                    {
                        tracing::error!(%trader, %order_id, "Failed to set matches to filled. Error: {e:#}");
                    }
                }
                Err(e) => {
                    // TODO(holzeis): If the order failed to execute, the matched limit order
                    // should also fail.
                    tracing::error!(%trader, %order_id, ?order_reason, "Failed to propose trade. Error: {e:#}");

                    if let Err(e) = spawn_blocking({
                        let pool = self.pool.clone();
                        move || {
                            let mut conn = pool.get()?;
                            matches::set_match_state(&mut conn, order_id, MatchState::Failed)?;
                            anyhow::Ok(())
                        }
                    })
                    .await?
                    {
                        tracing::error!(%trader, %order_id, "Failed to set matches to failed. Error: {e:#}");
                    }

                    bail!(e)
                }
            }
        } else {
            match order_reason {
                OrderReason::Manual => {
                    tracing::warn!(%trader, %order_id, ?order_reason, "Skipping trade execution as trader is not connected")
                }
                OrderReason::Expired
                | OrderReason::TraderLiquidated
                | OrderReason::CoordinatorLiquidated => {
                    tracing::info!(%trader, %order_id, ?order_reason, "Skipping trade execution as trader is not connected")
                }
            }
        }

        Ok(())
    }

    /// Checks if there are any pending matches
    async fn process_pending_match(&self, trader: PublicKey) -> Result<Option<ExecutableMatch>> {
        let matches = spawn_blocking({
            let pool = self.pool.clone();
            move || {
                let mut conn = pool.get().context("no connection")?;
                check_version(&mut conn, &trader)?;

                let matches = matches::get_pending_matches_by_trader(&mut conn, trader)?;
                anyhow::Ok(matches)
            }
        })
        .await??;

        let executable_match = if !matches.is_empty() {
            // we can assume that all matches belong to the same order id since a user
            // can only have one active order at the time. Meaning there can't
            // be multiple pending matches for different orders.
            let order_id = matches.first().expect("not empty list").order_id;
            let order = spawn_blocking({
                let pool = self.pool.clone();
                move || {
                    let mut conn = pool.get().context("no connection")?;
                    let order =
                        orders::get_with_id(&mut conn, order_id)?.context("Missing order")?;
                    anyhow::Ok(order)
                }
            })
            .await??;

            Some(ExecutableMatch { order, matches })
        } else {
            None
        };

        Ok(executable_match)
    }
}

pub struct MatchedOrder {
    pub order: Order,
    pub matched: Vec<Order>,
}

pub struct MatchExecutorSender {
    pub sender: mpsc::Sender<MatchedOrder>,
}

impl MatchExecutorSender {
    pub fn send(&self, matched_order: MatchedOrder) {
        tokio::spawn({
            let sender = self.sender.clone();
            async move {
                let trader = matched_order.order.trader_id;
                let order_id = matched_order.order.id;
                if let Err(e) = sender.send(matched_order).await {
                    tracing::error!(%trader, %order_id, "Failed to send trader message. Error: {e:#}");
                }
            }
        });
    }
}
