use crate::db;
use crate::node::decide_trade_action;
use crate::node::Node;
use crate::node::TradeAction;
use crate::position::models::Position;
use crate::position::models::PositionState;
use anyhow::Context;
use anyhow::Result;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::task::spawn_blocking;
use trade::bitmex_client::BitmexClient;

pub async fn sync_positions(node: Arc<Node>) -> Result<()> {
    let positions = spawn_blocking({
        let node = node.clone();
        move || get_open_positions(node)
    })
    .await
    .context("Failed to get open positions")??;

    for position in positions.iter() {
        tracing::debug!(trader_pk=%position.trader, %position.expiry_timestamp, "Attempting to close expired position");

        if !node.is_connected(&position.trader) {
            tracing::info!(
                "Could not close expired position with {} as trader is not connected.",
                position.trader
            );
            continue;
        }

        let channel_id = match spawn_blocking({
            let node = node.inner.clone();
            let trader = position.trader;
            move || decide_trade_action(node, &trader)
        })
        .await
        .expect("Failed to spawn blocking thread")
        {
            Ok(TradeAction::Close(channel_id)) => channel_id,
            Ok(_) => {
                tracing::error!(?position, "Unable to find sub channel of expired position.");
                continue;
            }
            Err(e) => {
                tracing::error!(?position, "Failed to decide trade action. Error: {e:?}");
                continue;
            }
        };

        let closing_price = match BitmexClient::get_quote(&position.expiry_timestamp).await {
            Ok(quote) => match position.direction {
                trade::Direction::Long => quote.bid_price,
                trade::Direction::Short => quote.ask_price,
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to get quote from bitmex for {} at {}. Error: {e:?}",
                    position.trader,
                    position.expiry_timestamp
                );
                continue;
            }
        };

        match node
            .close_position(position, closing_price, channel_id)
            .await
        {
            Ok(_) => tracing::info!(
                "Successfully proposed to close expired position with {}",
                position.trader
            ),
            Err(e) => tracing::warn!(
                ?position,
                "Failed to close expired position with {}. Error: {e:?}",
                position.trader
            ),
        }
    }
    Ok(())
}

fn get_open_positions(node: Arc<Node>) -> Result<Vec<Position>> {
    let mut conn = node.pool.get().context("Failed to get pool connection")?;
    let positions = db::positions::Position::get_all_open_positions(&mut conn)
        .context("Failed to get positions")?;

    let positions = positions
        .into_iter()
        .filter(|p| {
            p.position_state == PositionState::Open
                && OffsetDateTime::now_utc().ge(&p.expiry_timestamp)
        })
        .collect::<Vec<Position>>();
    Ok(positions)
}
