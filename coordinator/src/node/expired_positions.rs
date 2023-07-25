use crate::db;
use crate::node::Node;
use crate::node::TradeAction;
use crate::position::models::Position;
use crate::position::models::PositionState;
use hex::FromHex;
use lightning::ln::PaymentHash;
use time::OffsetDateTime;
use trade::bitmex_client::BitmexClient;

pub async fn close(node: Node) {
    let mut conn = match node.pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get pool connection. Error: {e:?}");
            return;
        }
    };

    let positions = match db::positions::Position::get_all_open_positions(&mut conn) {
        Ok(positions) => positions,
        Err(e) => {
            tracing::error!("Failed to get positions. Error: {e:?}");
            return;
        }
    };

    let positions = positions
        .into_iter()
        .filter(|p| {
            p.position_state == PositionState::Open
                && OffsetDateTime::now_utc().ge(&p.expiry_timestamp)
        })
        .collect::<Vec<Position>>();

    for position in positions.iter() {
        tracing::trace!(trader_pk=%position.trader, %position.expiry_timestamp, "Attempting to close expired position");

        if !node.is_connected(&position.trader) {
            tracing::debug!(
                "Could not close expired position with {} as trader is not connected.",
                position.trader
            );
            continue;
        }

        let channel_id = match node.decide_trade_action(&position.trader) {
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
            Ok(quote) => quote.get_price_for_direction(position.direction.opposite()),
            Err(e) => {
                tracing::warn!(
                    "Failed to get quote from bitmex for {} at {}. Error: {e:?}",
                    position.trader,
                    position.expiry_timestamp
                );
                continue;
            }
        };

        // Upon collab closing an expired position we cannot charge a fee using an
        // invoice. This dummy hash exists in the database to
        // represent zero-amount invoices.
        let zero_amount_payment_hash_dummy = PaymentHash(
            <[u8; 32]>::from_hex(
                "6f9b8c95c2ba7b1857b19f975372308161fedf50feb78a252200135a41875210",
            )
            .expect("static payment hash to decode"),
        );

        match node
            .close_position(
                position,
                closing_price,
                channel_id,
                zero_amount_payment_hash_dummy,
            )
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
}
