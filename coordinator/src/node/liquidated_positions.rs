use crate::db;
use crate::node::Node;
use crate::orderbook;
use crate::orderbook::db::orders;
use crate::orderbook::trading::NewOrderMessage;
use anyhow::Result;
use commons::average_execution_price;
use commons::Match;
use commons::MatchState;
use commons::NewMarketOrder;
use commons::OrderReason;
use commons::OrderState;
use commons::OrderType;
use commons::Price;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::ops::Add;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use trade::ContractSymbol;
use trade::Direction;

/// The timeout before we give up on closing a liquidated position collaboratively. This value
/// should not be larger than our refund transaction time lock.
pub const LIQUIDATION_POSITION_TIMEOUT: Duration = Duration::days(7);

/// The percentage of the liquidation price used as threshold at which the position will get
/// liquidated.
pub const MARGIN_CALL_PERCENTAGE: (i64, u32) = (10, 2);

pub async fn monitor(node: Node, trading_sender: mpsc::Sender<NewOrderMessage>) {
    if let Err(e) =
        check_if_positions_need_to_get_liquidated(trading_sender.clone(), node.clone()).await
    {
        tracing::error!("Failed to check if positions need to get liquidated. Error: {e:#}");
    }
}

/// Checks all open positions if given the best price the margin call has been reached and the
/// position needs to get liquidated. If so an async match is created and the user is notified about
/// the pending liquidation.
async fn check_if_positions_need_to_get_liquidated(
    trading_sender: mpsc::Sender<NewOrderMessage>,
    node: Node,
) -> Result<()> {
    let mut conn = node.pool.get()?;
    let open_positions = db::positions::Position::get_all_open_positions(&mut conn)?;
    let open_orders =
        orderbook::db::orders::get_all_orders(&mut conn, OrderType::Limit, OrderState::Open, true)?;

    if open_orders.is_empty() {
        tracing::warn!("No open orders found.");
        return Ok(());
    }

    let best_current_price = commons::best_current_price(&open_orders);
    let best_current_price = best_current_price
        .get(&ContractSymbol::BtcUsd)
        .expect("btc usd prices");

    for position in open_positions {
        let coordinator_liquidation_price =
            Decimal::try_from(position.coordinator_liquidation_price).expect("to fit into decimal");
        let trader_liquidation_price =
            Decimal::try_from(position.trader_liquidation_price).expect("to fit into decimal");

        if let Some(direction) = check_if_position_needs_to_get_liquidated(
            position.trader_direction,
            best_current_price,
            trader_liquidation_price,
            coordinator_liquidation_price,
        ) {
            if let Some(order) = orderbook::db::orders::get_by_trader_id_and_state(
                &mut conn,
                position.trader,
                OrderState::Matched,
            )? {
                let trader_id = order.trader_id.to_string();
                let order_id = order.id.to_string();

                if order.expiry < OffsetDateTime::now_utc() {
                    tracing::warn!(trader_id, order_id, "Matched order expired! Giving up on that position, looks like the corresponding dlc channel has to get force closed.");
                    orderbook::db::orders::set_order_state(
                        &mut conn,
                        order.id,
                        OrderState::Expired,
                    )?;

                    orderbook::db::matches::set_match_state_by_order_id(
                        &mut conn,
                        order.id,
                        MatchState::Failed,
                    )?;

                    let matches =
                        orderbook::db::matches::get_matches_by_order_id(&mut conn, order.id)?;
                    let matches: Vec<Match> = matches.into_iter().map(Match::from).collect();

                    let closing_price = average_execution_price(matches);
                    db::positions::Position::set_open_position_to_closing(
                        &mut conn,
                        &position.trader,
                        Some(closing_price),
                    )?;
                    continue;
                } else {
                    tracing::trace!(trader_id, order_id, "Skipping liquidated position as match has already been found. Waiting for trader to come online to execute the trade.");
                    continue;
                }
            }

            tracing::info!(trader_id=%position.trader, ?best_current_price, %direction, position_id=%position.id, "Attempting to close liquidated position");

            // Ensure that the users channel is confirmed on-chain before continuing with the
            // liquidation.
            match node
                .inner
                .is_signed_dlc_channel_confirmed_by_trader_id(position.trader)
            {
                Ok(true) => {
                    tracing::debug!(trader_id=%position.trader, "Traders dlc channel is confirmed. Continuing with the liquidation");
                }
                Ok(false) => {
                    tracing::warn!(trader_id=%position.trader, "Can't liquidated users position as the underlying channel is not yet confirmed");
                    continue;
                }
                Err(e) => {
                    tracing::error!(trader_id=%position.trader, "Failed to determine signed channel status. Skipping liquidation. Error: {e:#}");
                    continue;
                }
            }

            let new_order = NewMarketOrder {
                id: uuid::Uuid::new_v4(),
                contract_symbol: position.contract_symbol,
                quantity: Decimal::try_from(position.quantity).expect("to fit into decimal"),
                trader_id: position.trader,
                direction: position.trader_direction.opposite(),
                leverage: Decimal::from_f32(position.trader_leverage).expect("to fit into decimal"),
                // This order can basically not expire, but if the user does not come back online
                // within a certain time period we can assume the channel to be
                // abandoned and we should force close.
                expiry: OffsetDateTime::now_utc().add(LIQUIDATION_POSITION_TIMEOUT),
                stable: position.stable,
            };

            let order = match orders::insert_market_order(
                &mut conn,
                new_order.clone(),
                OrderReason::Liquidated,
            ) {
                Ok(order) => order,
                Err(e) => {
                    tracing::error!("Failed to insert liquidation order into DB. Error: {e:#}");
                    continue;
                }
            };

            let message = NewOrderMessage {
                order,
                channel_opening_params: None,
                order_reason: OrderReason::Liquidated,
            };

            if let Err(e) = trading_sender.send(message).await {
                tracing::error!(order_id=%new_order.id, trader_id=%new_order.trader_id, "Failed to submit new order for closing liquidated position. Error: {e:#}");
                continue;
            }
        }
    }

    Ok(())
}

/// Checks if the position needs to get liquidated. Either from the trader or the coordinator point
/// of view.
fn check_if_position_needs_to_get_liquidated(
    trader_direction: Direction,
    best_current_price: &Price,
    trader_liquidation_price: Decimal,
    coordinator_liquidation_price: Decimal,
) -> Option<Direction> {
    match trader_direction {
        Direction::Short => {
            // if the trader is short that means the coordinator is long, so we have to check the
            // coordinators liquidation price on the bid price.
            if let Some(bid) = best_current_price.bid {
                // check if coordinator needs to get liquidated
                if bid <= coordinator_liquidation_price {
                    return Some(Direction::Long);
                }
            }

            // if the trader is short that means the coordinator is long, so we have to check the
            // traders liquidation price on the ask price.
            if let Some(ask) = best_current_price.ask {
                // check if trader needs to get liquidated
                if ask >= trader_liquidation_price {
                    return Some(Direction::Short);
                }
            }
        }
        Direction::Long => {
            // if the trader is long that means the coordinator is short, so we have to check the
            // traders liquidation price on the bid price.
            if let Some(bid) = best_current_price.bid {
                // check if trader needs to get liquidated
                if bid <= trader_liquidation_price {
                    return Some(Direction::Long);
                }
            }

            // if the trader is long that means the coordinator is short, so we have to check the
            // coordinators liquidation price on the ask price.
            if let Some(ask) = best_current_price.ask {
                // check if coordinator needs to get liquidated
                if ask >= coordinator_liquidation_price {
                    return Some(Direction::Short);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::node::liquidated_positions::check_if_position_needs_to_get_liquidated;
    use commons::Price;
    use rust_decimal::Decimal;
    use trade::Direction;

    #[test]
    fn test_no_liquidatation_of_users_short_position_before_margin_call() {
        let trader_direction = Direction::Short;
        let price = Price {
            ask: Some(Decimal::from(33749)),
            bid: None,
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 5 / 4 = 37,500
            // margin call is at 10% of the liquidation price = 37,500 - 3,750 = 33,750
            Decimal::from(33750),
            // the liquidation price of the coordinator is at 30,000 * 5 / 6 = 25,000
            // margin call is at 10% of the liquidation price = 25,000 + 2,500 = 27,500
            Decimal::from(27500),
        );

        assert_eq!(None, liquidation);
    }

    #[test]
    fn test_liquidate_users_short_position_at_margin_call() {
        let trader_direction = Direction::Short;
        let price = Price {
            ask: Some(Decimal::from(33750)),
            bid: None,
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 5 / 4 = 37,500
            // margin call is at 10% of the liquidation price = 37,500 - 3,750 = 33,750
            Decimal::from(33750),
            // the liquidation price of the coordinator is at 30,000 * 5 / 6 = 25,000
            // margin call is at 10% of the liquidation price = 25,000 + 2,500 = 27,500
            Decimal::from(27500),
        );

        assert_eq!(Some(Direction::Short), liquidation);
    }

    #[test]
    fn test_liquidate_users_short_position_after_margin_call() {
        let trader_direction = Direction::Short;
        let price = Price {
            ask: Some(Decimal::from(33751)),
            bid: None,
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 5 / 4 = 37,500
            // margin call is at 10% of the liquidation price = 37,500 - 3,750 = 33,750
            Decimal::from(33750),
            // the liquidation price of the coordinator is at 30,000 * 5 / 6 = 25,000
            // margin call is at 10% of the liquidation price = 25,000 + 2,500 = 27,500
            Decimal::from(27500),
        );

        assert_eq!(Some(Direction::Short), liquidation);
    }

    #[test]
    fn test_no_liquidation_of_users_long_position_before_margin_call() {
        let trader_direction = Direction::Long;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(27501)),
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 5 / 6 = 25,000
            // margin call is at 10% of the liquidation price = 25,000 + 2,500 = 27,500
            Decimal::from(27500),
            // the liquidation price of the coordinator is at 30,000 * 5 / 4 = 37,500
            // margin call is at 10% of the liquidation price = 37,500 - 3,750 = 33,750
            Decimal::from(33750),
        );

        assert_eq!(None, liquidation);
    }

    #[test]
    fn test_liquidate_users_long_position_at_margin_call() {
        let trader_direction = Direction::Long;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(27500)),
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 5 / 6 = 25,000
            // margin call is at 10% of the liquidation price = 25,000 + 2,500 = 27,500
            Decimal::from(27500),
            // the liquidation price of the coordinator is at 30,000 * 5 / 4 = 37,500
            // margin call is at 10% of the liquidation price = 37,500 - 3,750 = 33,750
            Decimal::from(33750),
        );

        assert_eq!(Some(Direction::Long), liquidation);
    }

    #[test]
    fn test_liquidate_users_long_position_after_margin_call() {
        let trader_direction = Direction::Long;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(27499)),
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 5 / 6 = 25,000
            // margin call is at 10% of the liquidation price = 25,000 + 2,500 = 27,500
            Decimal::from(27500),
            // the liquidation price of the coordinator is at 30,000 * 5 / 4 = 37,500
            // margin call is at 10% of the liquidation price = 37,500 - 3,750 = 33,750
            Decimal::from(33750),
        );

        assert_eq!(Some(Direction::Long), liquidation);
    }

    #[test]
    fn test_liquidate_coordinators_short_position_at_margin_call() {
        let trader_direction = Direction::Long;
        let price = Price {
            ask: Some(Decimal::from(54000)),
            bid: None,
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 2 / 3 = 20,000
            // margin call is at 10% of the liquidation price = 20,000 + 2,000 = 22,000
            Decimal::from(22000),
            // the liquidation price of the coordinator is at 30,000 * 2 / 1 = 60,000
            // margin call is at 10% of the liquidation price = 60,000 - 6,000 = 54,000
            Decimal::from(54000),
        );

        assert_eq!(Some(Direction::Short), liquidation);
    }

    #[test]
    fn test_liquidate_coordinators_short_position_after_margin_call() {
        let trader_direction = Direction::Long;
        let price = Price {
            ask: Some(Decimal::from(54001)),
            bid: None,
        };

        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 2 / 3 = 20,000
            // margin call is at 10% of the liquidation price = 20,000 + 2,000 = 22,000
            Decimal::from(22000),
            // the liquidation price of the coordinator is at 30,000 * 2 / 1 = 60,000
            // margin call is at 10% of the liquidation price = 60,000 - 6,000 = 54,000
            Decimal::from(54000),
        );

        assert_eq!(Some(Direction::Short), liquidation);
    }

    #[test]
    fn test_no_liquidation_of_coordinators_short_position_before_margin_call() {
        let trader_direction = Direction::Long;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(53999)),
        };
        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 2 / 3 = 20,000
            // margin call is at 10% of the liquidation price = 20,000 + 2,000 = 22,000
            Decimal::from(22000),
            // the liquidation price of the coordinator is at 30,000 * 2 / 1 = 60,000
            // margin call is at 10% of the liquidation price = 60,000 - 6,000 = 54,000
            Decimal::from(54000),
        );

        assert_eq!(None, liquidation);
    }

    #[test]
    fn test_liquidate_coordinators_long_position_at_margin_call() {
        let trader_direction = Direction::Short;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(22000)),
        };
        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 2 / 1 = 60,000
            // margin call is at 10% of the liquidation price = 60,000 - 6,000 = 54,000
            Decimal::from(54000),
            // the liquidation price of the coordinator is at 30,000 * 2 / 3 = 20,000
            // margin call is at 10% of the liquidation price = 20,000 + 2,000 = 22,000
            Decimal::from(22000),
        );

        assert_eq!(Some(Direction::Long), liquidation);
    }

    #[test]
    fn test_liquidate_coordinators_long_position_after_margin_call() {
        let trader_direction = Direction::Short;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(21999)),
        };
        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 2 / 1 = 60,000
            // margin call is at 10% of the liquidation price = 60,000 - 6,000 = 54,000
            Decimal::from(54000),
            // the liquidation price of the coordinator is at 30,000 * 2 / 3 = 20,000
            // margin call is at 10% of the liquidation price = 20,000 + 2,000 = 22,000
            Decimal::from(22000),
        );

        assert_eq!(Some(Direction::Long), liquidation);
    }

    #[test]
    fn test_no_liquidation_of_coordinators_long_position_before_margin_call() {
        let trader_direction = Direction::Short;
        let price = Price {
            ask: None,
            bid: Some(Decimal::from(22001)),
        };
        let liquidation = check_if_position_needs_to_get_liquidated(
            trader_direction,
            &price,
            // the liquidation price of the trader is at 30,000 * 2 / 1 = 60,000
            // margin call is at 10% of the liquidation price = 60,000 - 6,000 = 54,000
            Decimal::from(54000),
            // the liquidation price of the coordinator is at 30,000 * 2 / 3 = 20,000
            // margin call is at 10% of the liquidation price = 20,000 + 2,000 = 22,000
            Decimal::from(22000),
        );

        assert_eq!(None, liquidation);
    }
}
