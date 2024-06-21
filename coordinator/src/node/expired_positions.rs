use crate::db;
use crate::node::Node;
use crate::orderbook;
use crate::orderbook::db::matches;
use crate::orderbook::trading::OrderbookMessage;
use crate::position::models::Position;
use crate::position::models::PositionState;
use anyhow::Context;
use anyhow::Result;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::ops::Add;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use xxi_node::commons::average_execution_price;
use xxi_node::commons::Match;
use xxi_node::commons::MatchState;
use xxi_node::commons::NewMarketOrder;
use xxi_node::commons::NewOrder;
use xxi_node::commons::OrderReason;

/// The timeout before we give up on closing an expired position collaboratively. This value should
/// not be larger than our refund transaction time lock.
pub const EXPIRED_POSITION_TIMEOUT: Duration = Duration::days(7);

pub async fn close(node: Node, orderbook_sender: mpsc::Sender<OrderbookMessage>) -> Result<()> {
    let mut conn = node.pool.get()?;

    let positions = db::positions::Position::get_all_open_positions(&mut conn)
        .context("Failed to fetch open positions")?;

    let positions = positions
        .into_iter()
        .filter(|p| {
            p.position_state == PositionState::Open
                && OffsetDateTime::now_utc().ge(&p.expiry_timestamp)
        })
        .collect::<Vec<Position>>();

    for position in positions.into_iter() {
        let matches = matches::get_pending_matches_by_trader(&mut conn, position.trader)?;
        if !matches.is_empty() {
            // we can assume that all matches belong to the same order id since a user can
            // only have one active order at the time. Meaning there can't be multiple pending
            // matches for different orders.
            let order_id = matches.first().expect("list not empty").order_id;
            let order = orderbook::db::orders::get_with_id(&mut conn, order_id)?
                .context("missing order")?;
            let trader = order.trader_id;

            if order.expiry < OffsetDateTime::now_utc() {
                tracing::warn!(%trader, %order_id, "Matched order expired! Giving up on that position, looks like the corresponding dlc channel has to get force closed.");

                matches::set_match_state(&mut conn, order_id, MatchState::Failed)?;

                let closing_price =
                    average_execution_price(matches.into_iter().map(Match::from).collect());
                db::positions::Position::set_open_position_to_closing(
                    &mut conn,
                    &position.trader,
                    Some(closing_price),
                )?;
                continue;
            } else {
                tracing::trace!(%trader, %order_id, "Skipping expired position as match has already been found. Waiting for trader to come online to execute the trade.");
                continue;
            }
        }

        tracing::debug!(trader_pk=%position.trader, %position.expiry_timestamp, "Attempting to close expired position");

        let order_id = uuid::Uuid::new_v4();
        let trader = position.trader;
        let new_order = NewMarketOrder {
            id: order_id,
            contract_symbol: position.contract_symbol,
            quantity: Decimal::try_from(position.quantity).expect("to fit into decimal"),
            trader_id: trader,
            direction: position.trader_direction.opposite(),
            leverage: Decimal::from_f32(position.trader_leverage).expect("to fit into decimal"),
            // This order can basically not expire, but if the user does not come back online within
            // a certain time period we can assume the channel to be abandoned and we should force
            // close.
            expiry: OffsetDateTime::now_utc().add(EXPIRED_POSITION_TIMEOUT),
            stable: position.stable,
        };

        let message = OrderbookMessage::NewOrder {
            new_order: NewOrder::Market(new_order),
            order_reason: OrderReason::Expired,
        };

        if let Err(e) = orderbook_sender.send(message).await {
            tracing::error!(%trader, %order_id, "Failed to submit new order for closing expired position. Error: {e:#}");
            continue;
        }
    }

    Ok(())
}
