use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use axum::Json;
use bdk::bitcoin::Transaction;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Amount;
use coordinator_commons::CollaborativeRevert;
use coordinator_commons::CollaborativeRevertData;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc::util::weight_to_fee;
use dlc_manager::subchannel::LNChannelManager;
use lightning::util::errors::APIError;
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

    let position =
        Position::get_position_by_trader(&mut conn, channel_details.counterparty.node_id, vec![])?
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

pub fn confirm_collaborative_revert(
    revert_params: &Json<CollaborativeRevertData>,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    channel_id: [u8; 32],
    inner_node: Arc<Node<NodeStorage>>,
) -> anyhow::Result<Transaction> {
    // TODO: check if provided amounts are as expected
    if !revert_params
        .transaction
        .output
        .iter()
        .any(|output| inner_node.wallet().is_mine(&output.script_pubkey).is_ok())
    {
        let error_message = "Invalid request: no address for coordinator provided".to_string();
        tracing::error!(error_message);
        bail!(error_message);
    }

    let sub_channels = inner_node
        .list_dlc_channels()
        .context("Failed to list dlc channels")?;
    let sub_channel = sub_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Could not find provided channel")?;

    let channel_manager = inner_node.channel_manager.clone();

    let mut own_sig = None;

    let mut revert_transaction = revert_params.transaction.clone();

    let position = Position::get_position_by_trader(conn, sub_channel.counter_party, vec![])?
        .context("Could not load position for channel_id")?;

    channel_manager
        .with_channel_lock_no_check(
            &sub_channel.channel_id,
            &sub_channel.counter_party,
            |channel_lock| {
                channel_manager.sign_with_fund_key_cb(channel_lock, &mut |fund_sk| {
                    let secp = Secp256k1::new();

                    own_sig = Some(
                        dlc::util::get_raw_sig_for_tx_input(
                            &secp,
                            &revert_transaction,
                            0,
                            &sub_channel.original_funding_redeemscript,
                            sub_channel.fund_value_satoshis,
                            fund_sk,
                        )
                        .expect("To be able to get raw sig for tx inpout"),
                    );

                    dlc::util::sign_multi_sig_input(
                        &secp,
                        &mut revert_transaction,
                        &revert_params.signature,
                        &sub_channel.counter_fund_pk,
                        fund_sk,
                        &sub_channel.original_funding_redeemscript,
                        sub_channel.fund_value_satoshis,
                        0,
                    )
                    .expect("To be able to sign multi sig");
                });
                Ok(())
            },
        )
        .map_err(|error| {
            let error = match error {
                APIError::APIMisuseError { .. } => "APIMisuseError",
                APIError::FeeRateTooHigh { .. } => "FeeRateTooHigh",
                APIError::InvalidRoute { .. } => "InvalidRoute",
                APIError::ChannelUnavailable { .. } => "ChannelUnavailable",
                APIError::MonitorUpdateInProgress => "MonitorUpdateInProgress",
                APIError::IncompatibleShutdownScript { .. } => "IncompatibleShutdownScript",
                APIError::ExternalError { .. } => "ExternalError",
            };
            tracing::error!("Could not get channel lock {error:#}");
            anyhow!("Could not get channel lock")
        })?;

    // if we have a sig here, it means we were able to sign the transaction and can broadcast it
    if own_sig.is_some() {
        inner_node
            .wallet()
            .broadcast_transaction(&revert_transaction)
            .context("Could not broadcast transaction")?;

        Position::set_position_to_closed(conn, position.id)
            .context("Could not set position to closed")?;

        Ok(revert_transaction)
    } else {
        bail!("Failed to sign revert transaction")
    }
}
