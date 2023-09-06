use crate::db;
use crate::node::Node;
use crate::orderbook;
use crate::orderbook::trading::NewOrderMessage;
use crate::orderbook::trading::TradingMessage;
use crate::position::models::Position;
use crate::position::models::PositionState;
use anyhow::Context;
use anyhow::Result;
use orderbook_commons::average_execution_price;
use orderbook_commons::Match;
use orderbook_commons::MatchState;
use orderbook_commons::NewOrder;
use orderbook_commons::Order;
use orderbook_commons::OrderState;
use orderbook_commons::OrderType;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::ops::Add;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::mpsc;

pub async fn close(node: Node, trading_sender: mpsc::Sender<TradingMessage>) -> Result<()> {
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
                orderbook::db::orders::set_order_state(&mut conn, order.id, OrderState::Failed)?;

                orderbook::db::matches::set_match_state_by_order_id(
                    &mut conn,
                    order.id,
                    MatchState::Failed,
                )?;

                let matches = orderbook::db::matches::get_matches_by_order_id(&mut conn, order.id)?;
                let matches: Vec<Match> = matches.into_iter().map(Match::from).collect();

                let closing_price = average_execution_price(matches)
                    .to_f32()
                    .expect("to fit into f32");
                db::positions::Position::set_open_position_to_closing(
                    &mut conn,
                    position.trader.to_string(),
                    closing_price,
                )?;
                continue;
            } else {
                tracing::trace!(trader_id, order_id, "Skipping expired position as match has already been found. Waiting for trader to come online to execute the trade.");
                continue;
            }
        }

        tracing::debug!(trader_pk=%position.trader, %position.expiry_timestamp, "Attempting to close expired position");

        let new_order = NewOrder {
            id: uuid::Uuid::new_v4(),
            contract_symbol: position.contract_symbol,
            // todo(holzeis): we should not have to set the price for a market order. we propably
            // need separate models for a limit and a market order.
            price: Decimal::ZERO,
            quantity: Decimal::try_from(position.quantity).expect("to fit into decimal"),
            trader_id: position.trader,
            direction: position.direction.opposite(),
            leverage: position.leverage,
            order_type: OrderType::Market,
            // This order can basically not expire, but if the user does not come back online within
            // 8 weeks I guess we can safely assume that this channel is abandoned and we should
            // consider force closing.
            expiry: OffsetDateTime::now_utc().add(Duration::weeks(8)),
        };

        let (sender, mut receiver) = mpsc::channel::<Result<Order>>(1);
        let message = TradingMessage::NewOrder(NewOrderMessage {
            new_order: new_order.clone(),
            sender,
        });

        if let Err(e) = trading_sender.send(message).await {
            tracing::error!(order_id=%new_order.id, trader_id=%new_order.trader_id, "Failed to submit new order for closing expired position. Error: {e:#}");
            continue;
        }

        match receiver.recv().await {
            Some(Ok(order)) => order,
            Some(Err(e)) => {
                tracing::error!(order_id=%new_order.id, trader_id=%new_order.trader_id, "Failed to submit new order for closing expired position. Error: {e:#}");
                continue;
            }
            None => {
                tracing::error!(order_id=%new_order.id, trader_id=%new_order.trader_id, "Failed to receive response from trading.");
                continue;
            }
        };
    }

    Ok(())
}
