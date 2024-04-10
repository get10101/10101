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

pub async fn monitor(node: Node, trading_sender: mpsc::Sender<NewOrderMessage>) {
    if let Err(e) =
        check_if_positions_need_to_get_liquidated(trading_sender.clone(), node.clone()).await
    {
        tracing::error!("Failed to check if positions need to get liquidated. Error: {e:#}");
    }
}

/// For all open positions, check if the maintenance margin has been reached. Send a liquidation
/// async match to the traders whose positions have been liquidated.
async fn check_if_positions_need_to_get_liquidated(
    trading_sender: mpsc::Sender<NewOrderMessage>,
    node: Node,
) -> Result<()> {
    let mut conn = node.pool.get()?;
    let open_positions = db::positions::Position::get_all_open_positions(&mut conn)?;
    let best_current_price =
        orderbook::db::orders::get_best_price(&mut conn, ContractSymbol::BtcUsd)?;

    for position in open_positions {
        let coordinator_liquidation_price =
            Decimal::try_from(position.coordinator_liquidation_price).expect("to fit into decimal");
        let trader_liquidation_price =
            Decimal::try_from(position.trader_liquidation_price).expect("to fit into decimal");

        let trader_liquidation = check_if_position_needs_to_get_liquidated(
            position.trader_direction,
            &best_current_price,
            trader_liquidation_price,
        );
        let coordinator_liquidation = check_if_position_needs_to_get_liquidated(
            position.trader_direction.opposite(),
            &best_current_price,
            coordinator_liquidation_price,
        );

        if trader_liquidation || coordinator_liquidation {
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

            tracing::info!(trader_id=%position.trader, ?best_current_price, position_id=%position.id, "Attempting to close liquidated position");

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

            let order_reason = match trader_liquidation {
                true => OrderReason::TraderLiquidated,
                false => OrderReason::CoordinatorLiquidated,
            };

            let order = match orders::insert_market_order(
                &mut conn,
                new_order.clone(),
                order_reason.clone(),
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
                order_reason,
            };

            if let Err(e) = trading_sender.send(message).await {
                tracing::error!(order_id=%new_order.id, trader_id=%new_order.trader_id, "Failed to submit new order for closing liquidated position. Error: {e:#}");
                continue;
            }
        }
    }

    Ok(())
}

fn check_if_position_needs_to_get_liquidated(
    direction: Direction,
    best_current_price: &Price,
    liquidation_price: Decimal,
) -> bool {
    match direction {
        Direction::Short => best_current_price
            .ask
            .map(|ask| ask >= liquidation_price)
            .unwrap_or(false),
        Direction::Long => best_current_price
            .bid
            .map(|bid| bid <= liquidation_price)
            .unwrap_or(false),
    }
}
