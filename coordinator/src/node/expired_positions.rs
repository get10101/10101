use crate::db;
use crate::node::Node;
use crate::orderbook;
use crate::orderbook::db::orders;
use crate::orderbook::trading::NewOrderMessage;
use crate::position::models::Position;
use crate::position::models::PositionState;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use commons::average_execution_price;
use commons::Match;
use commons::MatchState;
use commons::NewMarketOrder;
use commons::OrderReason;
use commons::OrderState;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::ops::Add;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::mpsc;

/// The timeout before we give up on closing an expired position collaboratively. This value should
/// not be larger than our refund transaction time lock.
pub const EXPIRED_POSITION_TIMEOUT: Duration = Duration::days(7);

pub async fn close(node: Node, trading_sender: mpsc::Sender<NewOrderMessage>) -> Result<()> {
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
        if let Some(order) = orderbook::db::orders::get_by_trader_id_and_state(
            &mut conn,
            position.trader,
            OrderState::Matched,
        )? {
            let trader_id = order.trader_id.to_string();
            let order_id = order.id.to_string();

            if order.expiry < OffsetDateTime::now_utc() {
                tracing::warn!(trader_id, order_id, "Matched order expired! Giving up on that position, looks like the corresponding dlc channel has to get force closed.");
                orderbook::db::orders::set_order_state(&mut conn, order.id, OrderState::Expired)?;

                orderbook::db::matches::set_match_state_by_order_id(
                    &mut conn,
                    order.id,
                    MatchState::Failed,
                )?;

                let matches = orderbook::db::matches::get_matches_by_order_id(&mut conn, order.id)?;
                let matches: Vec<Match> = matches.into_iter().map(Match::from).collect();

                db::positions::Position::set_open_position_to_closing(
                    &mut conn,
                    &position.trader,
                    Some(average_execution_price(matches)),
                )?;
                continue;
            } else {
                tracing::trace!(trader_id, order_id, "Skipping expired position as match has already been found. Waiting for trader to come online to execute the trade.");
                continue;
            }
        }

        tracing::debug!(trader_pk=%position.trader, %position.expiry_timestamp, "Attempting to close expired position");

        let new_order = NewMarketOrder {
            id: uuid::Uuid::new_v4(),
            contract_symbol: position.contract_symbol,
            quantity: Decimal::try_from(position.quantity).expect("to fit into decimal"),
            trader_id: position.trader,
            direction: position.trader_direction.opposite(),
            leverage: Decimal::from_f32(position.trader_leverage).expect("to fit into decimal"),
            // This order can basically not expire, but if the user does not come back online within
            // a certain time period we can assume the channel to be abandoned and we should force
            // close.
            expiry: OffsetDateTime::now_utc().add(EXPIRED_POSITION_TIMEOUT),
            stable: position.stable,
        };

        let order = orders::insert_market_order(&mut conn, new_order.clone(), OrderReason::Expired)
            .map_err(|e| anyhow!(e))
            .context("Failed to insert expired order into DB")?;

        let message = NewOrderMessage {
            order,
            channel_opening_params: None,
            order_reason: OrderReason::Expired,
        };

        if let Err(e) = trading_sender.send(message).await {
            tracing::error!(order_id=%new_order.id, trader_id=%new_order.trader_id, "Failed to submit new order for closing expired position. Error: {e:#}");
            continue;
        }
    }

    Ok(())
}
