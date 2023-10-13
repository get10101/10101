use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::position;
use anyhow::anyhow;
use anyhow::Context;
use axum::Json;
use bitcoin::Amount;
use coordinator_commons::CollaborativeRevert;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc::util::weight_to_fee;
use dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::Node;
use orderbook_commons::Message;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use trade::bitmex_client::Quote;

/// The weight for the collaborative close transaction. It's expected to have 1 input (from the fund
/// transaction) and 2 outputs, one for each party.
/// Note: if either party would have a 0 output, the actual weight will be smaller and we will be
/// overspending tx fee.
const COLLABORATIVE_REVERT_TX_WEIGHT: usize = 672;

pub async fn notify_user_to_collaboratively_revert(
    revert_params: Json<CollaborativeRevert>,
    channel_id_string: String,
    channel_id: [u8; 32],
    pool: Pool<ConnectionManager<PgConnection>>,
    node: Arc<Node<NodeStorage>>,
    auth_users_notifier: mpsc::Sender<OrderbookMessage>,
) -> anyhow::Result<()> {
    let mut conn = pool.get().context("Could not acquire db lock")?;

    let channel_details = node
        .channel_manager
        .get_channel_details(&channel_id)
        .context("Could not get channel")?;

    let sub_channels = node
        .list_dlc_channels()
        .context("Could not list dlc channels")?;

    let sub_channel = sub_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Could not find provided channel")?;

    let position = Position::get_position_by_channel_id(&mut conn, channel_id_string.clone())
        .context("Could not load position for channel_id")?;

    let settlement_amount = position
        .calculate_settlement_amount(revert_params.price)
        .context("Could not calculate settlement amount")?;

    let pnl = position
        .calculate_coordinator_pnl(Quote {
            bid_size: 0,
            ask_size: 0,
            bid_price: revert_params.price,
            ask_price: revert_params.price,
            symbol: "".to_string(),
            timestamp: OffsetDateTime::now_utc(),
        })
        .context("Could not calculate coordinator pnl")?;

    // There is no easy way to get the total tx fee for all subchannel transactions, hence, we
    // estimate it. This transaction fee is shared among both users fairly
    let dlc_channel_fee = calculate_dlc_channel_tx_fees(
        sub_channel.fund_value_satoshis,
        pnl,
        channel_details.inbound_capacity_msat / 1000,
        channel_details.outbound_capacity_msat / 1000,
        position.trader_margin,
        position.coordinator_margin,
    );

    // Coordinator's amount is the total channel's value (fund_value_satoshis) whatever the taker
    // had (inbound_capacity), the taker's PnL (settlement_amount) and the transaction fee
    let coordinator_amount = sub_channel.fund_value_satoshis as i64
        - (channel_details.inbound_capacity_msat / 1000) as i64
        - settlement_amount as i64
        - (dlc_channel_fee as f64 / 2.0) as i64;
    let trader_amount = sub_channel.fund_value_satoshis - coordinator_amount as u64;

    let fee = weight_to_fee(
        COLLABORATIVE_REVERT_TX_WEIGHT,
        revert_params.fee_rate_sats_vb,
    )
    .expect("To be able to calculate constant fee rate");

    let coordinator_addrss = node.get_unused_address();
    let coordinator_amount = Amount::from_sat(coordinator_amount as u64 - fee / 2);
    let trader_amount = Amount::from_sat(trader_amount - fee / 2);

    // TODO: check if trader still has more than dust
    tracing::info!(
        channel_id = channel_id_string,
        coordinator_address = %coordinator_addrss,
        coordinator_amount = coordinator_amount.to_sat(),
        trader_amount = trader_amount.to_sat(),
        "Proposing collaborative revert");

    db::collaborative_reverts::insert(
        &mut conn,
        position::models::CollaborativeRevert {
            channel_id,
            trader_pubkey: position.trader,
            price: revert_params.price.to_f32().expect("to fit into f32"),
            coordinator_address: coordinator_addrss.clone(),
            coordinator_amount_sats: coordinator_amount,
            trader_amount_sats: trader_amount,
            timestamp: OffsetDateTime::now_utc(),
        },
    )
    .context("Could not insert new collaborative revert")?;

    // try to notify user
    let sender = auth_users_notifier;
    sender
        .send(OrderbookMessage::CollaborativeRevert {
            trader_id: position.trader,
            message: Message::CollaborativeRevert {
                channel_id,
                coordinator_address: coordinator_addrss,
                coordinator_amount,
                trader_amount,
            },
        })
        .await
        .map_err(|error| anyhow!("Could send message to notify user {error:#}"))?;
    Ok(())
}

fn calculate_dlc_channel_tx_fees(
    initial_funding: u64,
    pnl: i64,
    inbound_capacity: u64,
    outbound_capacity: u64,
    trader_margin: i64,
    coordinator_margin: i64,
) -> u64 {
    initial_funding
        - (inbound_capacity
            + outbound_capacity
            + (trader_margin - pnl) as u64
            + (coordinator_margin + pnl) as u64)
}

#[cfg(test)]
pub mod tests {
    use crate::collaborative_revert::calculate_dlc_channel_tx_fees;

    #[test]
    pub fn calculate_transaction_fee_for_dlc_channel_transactions() {
        let total_fee =
            calculate_dlc_channel_tx_fees(200_000, -4047, 65_450, 85_673, 18_690, 18_690);
        assert_eq!(total_fee, 11_497);
    }
}
