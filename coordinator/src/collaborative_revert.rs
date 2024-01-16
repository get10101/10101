use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::notifications::NotificationKind;
use crate::position;
use crate::storage::CoordinatorTenTenOneStorage;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Amount;
use bitcoin::Transaction;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc::util::weight_to_fee;
use dlc_manager::channel::ClosedChannel;
use dlc_manager::DlcChannelId;
use dlc_manager::Signer;
use dlc_manager::Storage;
use ln_dlc_node::node::Node;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::mpsc;

/// The weight for the collaborative revert transaction. The transaction is expected to have 1 input
/// (the funding TXO) and 2 outputs, one for each party.
///
/// If either party were to _not_ have an output, we would be overestimating the weight of the
/// transaction and would end up paying higher fees than necessary.
const COLLABORATIVE_REVERT_TX_WEIGHT: usize = 672;

/// Propose collaboratively reverting the channel identified by `channel_id`.
///
/// A collaborative revert involves signing a new transaction spending from the funding output
/// directly. This can be used to circumvent bugs related to position and subchannel state.
///
/// This API will only work if the DlcChannel is in state [`Channel::Signed`].
#[allow(clippy::too_many_arguments)]
pub async fn propose_collaborative_revert(
    node: Arc<Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    pool: Pool<ConnectionManager<PgConnection>>,
    sender: mpsc::Sender<OrderbookMessage>,
    channel_id: DlcChannelId,
    fee_rate_sats_vb: u64,
    trader_amount_sats: u64,
    closing_price: Decimal,
) -> Result<()> {
    let mut conn = pool.get().context("Could not acquire DB lock")?;

    let channel_id_hex = channel_id.to_hex();

    let dlc_channels = node
        .list_signed_dlc_channels()
        .context("Could not get list of subchannels")?;

    let channel = dlc_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("DLC channel not found")?;

    let peer_id = channel.counter_party;

    let fund_tx_output = channel
        .fund_tx
        .output
        .get(channel.fund_output_index)
        .expect("to be the correct index");

    let coordinator_amount_sats = fund_tx_output
        .value
        .checked_sub(trader_amount_sats)
        .context("could not substract trader amount from total value without overflow")?;

    let fee = weight_to_fee(COLLABORATIVE_REVERT_TX_WEIGHT, fee_rate_sats_vb)
        .expect("To be able to calculate constant fee rate");

    let fee_half = fee.checked_div(2).context("Could not divide fee")?;

    let coordinator_address = node.get_unused_address();
    let coordinator_amount = Amount::from_sat(
        coordinator_amount_sats
            .checked_sub(fee_half)
            .context("Could not subtract fee from coordinator amount")?,
    );

    let trader_amount = Amount::from_sat(
        trader_amount_sats
            .checked_sub(fee_half)
            .context("Could not subtract fee from trader amount")?,
    );

    tracing::info!(
        channel_id = channel_id_hex,
        coordinator_address = %coordinator_address,
        coordinator_amount = %coordinator_amount,
        trader_amount = %trader_amount,
        "Proposing collaborative revert"
    );

    db::collaborative_reverts::insert(
        &mut conn,
        position::models::CollaborativeRevert {
            channel_id,
            trader_pubkey: peer_id,
            coordinator_address: coordinator_address.clone(),
            coordinator_amount_sats: coordinator_amount,
            trader_amount_sats: trader_amount,
            timestamp: OffsetDateTime::now_utc(),
            price: closing_price,
        },
    )
    .context("Could not insert new collaborative revert")?;

    // Send collaborative revert proposal to the counterpary.
    sender
        .send(OrderbookMessage::TraderMessage {
            trader_id: peer_id,
            message: Message::CollaborativeRevert {
                channel_id,
                coordinator_address,
                coordinator_amount,
                trader_amount,
                execution_price: closing_price,
            },
            notification: Some(NotificationKind::CollaborativeRevert),
        })
        .await
        .map_err(|error| anyhow!("Could send message to notify user {error:#}"))?;

    Ok(())
}

/// Complete the collaborative revert protocol by:
///
/// 1. Verifying the contents of the transaction sent by the counterparty.
/// 2. Signing the transaction.
/// 3. Broadcasting the signed transaction.
pub fn confirm_collaborative_revert(
    node: Arc<Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    channel_id: DlcChannelId,
    revert_transaction: Transaction,
    counterparty_signature: Signature,
) -> Result<Transaction> {
    let channel_id_hex = channel_id.to_hex();

    let record =
        db::collaborative_reverts::get_by_channel_id(conn, &channel_id)?.with_context(|| {
            format!(
                "No matching record to confirm collaborative revert for channel {channel_id_hex}"
            )
        })?;

    tracing::info!(
        collaborative_revert_record = ?record,
        "Confirming collaborative revert"
    );

    // TODO: Check if provided amounts are as expected.

    let does_revert_pay_to_coordinator = revert_transaction.output.iter().any(|output| {
        node.wallet()
            .is_mine(&output.script_pubkey)
            .unwrap_or_else(|e| {
                tracing::error!(
                    "Failed to confirm if proposed collaborative revert \
                     transaction pays to the coordinator: {e:#}"
                );
                false
            })
    });

    ensure!(
        does_revert_pay_to_coordinator,
        "Proposed collaborative revert transaction doesn't pay the coordinator"
    );

    let signed_channel = node
        .list_signed_dlc_channels()
        .context("Failed to list signed dlc channels")?;
    let signed_channel = signed_channel
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("DLC channel to be reverted not found")?;

    let fund_out_amount = signed_channel.fund_tx.output[signed_channel.fund_output_index].value;

    let own_fund_sk = node
        .wallet()
        .get_secret_key_for_pubkey(&signed_channel.own_params.fund_pubkey)?;

    let mut close_tx = revert_transaction.clone();

    dlc::util::sign_multi_sig_input(
        &Secp256k1::new(),
        &mut close_tx,
        &counterparty_signature,
        &signed_channel.counter_params.fund_pubkey,
        &own_fund_sk,
        &signed_channel.fund_script_pubkey,
        fund_out_amount,
        0,
    )?;

    tracing::info!(
        txid = close_tx.txid().to_string(),
        "Broadcasting collaborative revert transaction"
    );
    node.ldk_wallet()
        .broadcast_transaction(&close_tx)
        .context("Could not broadcast transaction")?;

    // TODO: We should probably not modify the state until the transaction has been confirmed.

    let position = Position::get_position_by_trader(conn, record.trader_pubkey, vec![])?
        .with_context(|| format!("Could not load position for subchannel {channel_id_hex}"))?;

    Position::set_position_to_closed(conn, position.id)
        .context("Could not set position to closed")?;

    db::collaborative_reverts::delete(conn, channel_id)?;

    node.dlc_manager
        .get_store()
        .upsert_channel(
            dlc_manager::channel::Channel::CollaborativelyClosed(ClosedChannel {
                counter_party: signed_channel.counter_party,
                temporary_channel_id: signed_channel.temporary_channel_id,
                channel_id: signed_channel.channel_id,
            }),
            // The contract doesn't matter anymore
            None,
        )
        .map_err(|e| anyhow!("{e:#}"))?;

    Ok(revert_transaction)
}
