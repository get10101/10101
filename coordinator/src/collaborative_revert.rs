use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::notifications::NotificationKind;
use crate::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use axum::Json;
use bdk::bitcoin::Transaction;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Amount;
use bitcoin::OutPoint;
use coordinator_commons::CollaborativeRevert;
use coordinator_commons::CollaborativeRevertData;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc::util::weight_to_fee;
use dlc_manager::subchannel::LNChannelManager;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use lightning::chain::keysinterface::ExtraSign;
use lightning::chain::keysinterface::SignerProvider;
use ln_dlc_node::node::Node;
use orderbook_commons::Message;
use rust_decimal::prelude::ToPrimitive;
use std::ops::Deref;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::mpsc;

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
    funding_outpoint: OutPoint,
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

    let trade_settled = sub_channel.fund_value_satoshis == channel_details.channel_value_satoshis;

    // There is no easy way to get the total tx fee for all subchannel transactions, hence, we
    // estimate it. This transaction fee is shared among both users fairly
    let dlc_channel_fee = calculate_dlc_channel_tx_fees(
        trade_settled,
        sub_channel.fund_value_satoshis,
        channel_details.inbound_capacity_msat / 1000,
        channel_details.outbound_capacity_msat / 1000,
        position.trader_margin as u64,
        position.coordinator_margin as u64,
        channel_details.unspendable_punishment_reserve.unwrap_or(0),
    )?;

    // Coordinator's amount is the total channel's value (fund_value_satoshis) whatever the taker
    // had (inbound_capacity), the taker's PnL (settlement_amount) and the transaction fee
    let coordinator_amount = match trade_settled {
        false => {
            sub_channel.fund_value_satoshis as i64
                - (channel_details.inbound_capacity_msat / 1000) as i64
                - settlement_amount as i64
                - (dlc_channel_fee as f64 / 2.0) as i64
                - channel_details.unspendable_punishment_reserve.unwrap_or(0) as i64
        }
        true => {
            sub_channel.fund_value_satoshis as i64
                - (channel_details.inbound_capacity_msat / 1000) as i64
                - channel_details.unspendable_punishment_reserve.unwrap_or(0) as i64
        }
    };
    let trader_amount = sub_channel.fund_value_satoshis - coordinator_amount as u64;

    let fee = weight_to_fee(
        COLLABORATIVE_REVERT_TX_WEIGHT,
        revert_params.fee_rate_sats_vb,
    )
    .expect("To be able to calculate constant fee rate");

    tracing::debug!(
        coordinator_amount,
        fund_value_satoshis = sub_channel.fund_value_satoshis,
        inbound_capacity_msat = channel_details.inbound_capacity_msat,
        settlement_amount,
        dlc_channel_fee,
        inbound_capacity_msat = channel_details.inbound_capacity_msat,
        outbound_capacity_msat = channel_details.outbound_capacity_msat,
        trader_margin = position.trader_margin,
        coordinator_margin = position.coordinator_margin,
        position_id = position.id,
        "Collaborative revert temporary values"
    );

    let coordinator_address = node.get_unused_address();
    let coordinator_amount = Amount::from_sat(coordinator_amount as u64 - fee / 2);
    let trader_amount = Amount::from_sat(trader_amount - fee / 2);

    // TODO: check if trader still has more than dust
    tracing::info!(
        channel_id = channel_id_string,
        coordinator_address = %coordinator_address,
        coordinator_amount = coordinator_amount.to_sat(),
        trader_amount = trader_amount.to_sat(),
        "Proposing collaborative revert");

    db::collaborative_reverts::insert(
        &mut conn,
        position::models::CollaborativeRevert {
            channel_id,
            trader_pubkey: position.trader,
            price: revert_params.price.to_f32().expect("to fit into f32"),
            coordinator_address: coordinator_address.clone(),
            coordinator_amount_sats: coordinator_amount,
            trader_amount_sats: trader_amount,
            timestamp: OffsetDateTime::now_utc(),
            txid: funding_outpoint.txid,
            vout: funding_outpoint.vout,
        },
    )
    .context("Could not insert new collaborative revert")?;

    // try to notify user
    let sender = auth_users_notifier;
    sender
        .send(OrderbookMessage::TraderMessage {
            trader_id: position.trader,
            message: Message::CollaborativeRevert {
                channel_id,
                coordinator_address,
                coordinator_amount,
                trader_amount,
                execution_price: revert_params.price,
                outpoint: funding_outpoint,
            },
            notification: Some(NotificationKind::CollaborativeRevert),
        })
        .await
        .map_err(|error| anyhow!("Could send message to notify user {error:#}"))?;
    Ok(())
}

fn calculate_dlc_channel_tx_fees(
    trade_settled: bool,
    sub_channel_sats: u64,
    inbound_capacity: u64,
    outbound_capacity: u64,
    trader_margin: u64,
    coordinator_margin: u64,
    reserve: u64,
) -> anyhow::Result<u64> {
    let mut dlc_tx_fee = sub_channel_sats
        .checked_sub(inbound_capacity)
        .context("could not subtract inbound capacity")?
        .checked_sub(outbound_capacity)
        .context("could not subtract outbound capacity")?
        .checked_sub(reserve * 2)
        .context("could not substract the reserve")?;

    if !trade_settled {
        // the ln channel has not yet been updated so we need to take the margins into account.
        dlc_tx_fee = dlc_tx_fee
            .checked_sub(trader_margin)
            .context("could not subtract trader margin")?
            .checked_sub(coordinator_margin)
            .context("could not subtract coordinator margin")?;
    }

    Ok(dlc_tx_fee)
}

#[cfg(test)]
pub mod tests {
    use crate::collaborative_revert::calculate_dlc_channel_tx_fees;

    #[test]
    pub fn calculate_transaction_fee_for_dlc_channel_transactions_with_smaller_ln_channel() {
        let total_fee =
            calculate_dlc_channel_tx_fees(false, 200_000, 65_450, 85_673, 18_690, 18_690, 1_000)
                .unwrap();
        assert_eq!(total_fee, 9_497);
    }

    #[test]
    pub fn calculate_transaction_fee_for_dlc_channel_transactions_with_equal_ln_channel() {
        let total_fee =
            calculate_dlc_channel_tx_fees(true, 200_000, 84_140, 104_363, 18_690, 18_690, 1_000)
                .unwrap();
        assert_eq!(total_fee, 9_497);
    }

    #[test]
    pub fn ensure_overflow_being_caught() {
        assert!(calculate_dlc_channel_tx_fees(
            false, 200_000, 84_140, 104_363, 18_690, 18_690, 1_000
        )
        .is_err());
    }
}

pub fn confirm_collaborative_revert(
    revert_params: &Json<CollaborativeRevertData>,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    channel_id: [u8; 32],
    inner_node: Arc<Node<NodeStorage>>,
) -> anyhow::Result<Transaction> {
    tracing::debug!(
        channel_id = revert_params.channel_id,
        txid = revert_params.transaction.txid().to_string(),
        "Confirming collaborative revert"
    );
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

    let mut revert_transaction = revert_params.transaction.clone();

    let position = Position::get_position_by_trader(conn, sub_channel.counter_party, vec![])?
        .context("Could not load position for channel_id")?;

    let channel_value = sub_channel.fund_value_satoshis;

    let previous_outpoint = revert_transaction
        .input
        .get(0)
        .context("Revert transaction didn't have an input")?
        .previous_output;
    let monitor = inner_node
        .chain_monitor
        .get_monitor(lightning::chain::transaction::OutPoint {
            txid: previous_outpoint.txid,
            index: previous_outpoint.vout as u16,
        })
        .map_err(|_| anyhow!("Could not get chain monitor"))?;
    let channel_monitor = monitor.deref();
    let user_channel_keys = channel_monitor
        .inner
        .lock()
        .map_err(|_| anyhow!("Could not acquire channel monitor lock"))?
        .channel_keys_id;

    let signer = inner_node
        .keys_manager
        .derive_channel_signer(channel_value, user_channel_keys);

    let mut own_sig = None;
    signer.sign_with_fund_key_callback(&mut |key| {
        let secp = Secp256k1::new();
        own_sig = Some(
            dlc::util::get_raw_sig_for_tx_input(
                &secp,
                &revert_transaction,
                0,
                &sub_channel.original_funding_redeemscript,
                channel_value,
                key,
            )
            .expect("To be able to get raw sig for tx input"),
        );

        dlc::util::sign_multi_sig_input(
            &secp,
            &mut revert_transaction,
            &revert_params.signature,
            &sub_channel.counter_fund_pk,
            key,
            &sub_channel.original_funding_redeemscript,
            channel_value,
            0,
        )
        .expect("To be able to sign multi sig");
    });

    // if we have a sig here, it means we were able to sign the transaction and can broadcast it
    if own_sig.is_some() {
        tracing::info!(
            txid = revert_transaction.txid().to_string(),
            "Broadcasting collaborative revert transaction"
        );
        inner_node
            .wallet()
            .broadcast_transaction(&revert_transaction)
            .context("Could not broadcast transaction")?;

        Position::set_position_to_closed(conn, position.id)
            .context("Could not set position to closed")?;

        let mut sub_channel = sub_channel.clone();

        sub_channel.state = SubChannelState::OnChainClosed;
        inner_node
            .sub_channel_manager
            .get_dlc_manager()
            .get_store()
            .upsert_sub_channel(&sub_channel)?;

        db::collaborative_reverts::delete(conn, channel_id)?;

        Ok(revert_transaction)
    } else {
        bail!("Failed to sign revert transaction")
    }
}
