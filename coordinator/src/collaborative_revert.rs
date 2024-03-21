use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::notifications::NotificationKind;
use crate::position;
use crate::storage::CoordinatorTenTenOneStorage;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Transaction;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc::util::tx_weight_to_fee;
use dlc_manager::channel::ClosedChannel;
use dlc_manager::DlcChannelId;
use dlc_manager::Signer;
use dlc_manager::Storage;
use futures::executor::block_on;
use ln_dlc_node::bitcoin_conversion::to_ecdsa_signature_29;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_30;
use ln_dlc_node::bitcoin_conversion::to_tx_29;
use ln_dlc_node::bitcoin_conversion::to_tx_30;
use ln_dlc_node::bitcoin_conversion::to_txid_29;
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

/// Propose to collaboratively revert the channel identified by `channel_id`.
///
/// A collaborative revert involves signing a new transaction spending from the funding output
/// directly. This can be used to circumvent bugs related to position and DLC channel state.
///
/// This API will only work if the DLC [`Channel`] is in state [`Channel::Signed`].
#[allow(clippy::too_many_arguments)]
pub async fn propose_collaborative_revert(
    node: Arc<
        Node<
            bdk_file_store::Store<bdk::wallet::ChangeSet>,
            CoordinatorTenTenOneStorage,
            NodeStorage,
        >,
    >,
    pool: Pool<ConnectionManager<PgConnection>>,
    sender: mpsc::Sender<OrderbookMessage>,
    channel_id: DlcChannelId,
    fee_rate_sats_vb: u64,
    trader_amount_sats: u64,
    closing_price: Decimal,
) -> Result<()> {
    let channel_id_hex = hex::encode(channel_id);

    let dlc_channels = node
        .list_signed_dlc_channels()
        .context("Could not get list of subchannels")?;

    let channel = dlc_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Could not find signed DLC channel")?;

    let peer_id = channel.counter_party;

    let fund_tx_output = channel
        .fund_tx
        .output
        .get(channel.fund_output_index)
        .expect("to be the correct index");

    let coordinator_amount_sats = fund_tx_output
        .value
        .checked_sub(trader_amount_sats)
        .context("Could not substract trader amount from total value without overflow")?;

    let fee = tx_weight_to_fee(COLLABORATIVE_REVERT_TX_WEIGHT, fee_rate_sats_vb)
        .context("Could not calculate fee")?;

    let fee_half = fee.checked_div(2).context("Could not divide fee")?;

    let coordinator_address = node.get_new_address()?;
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

    {
        let mut conn = pool.get().context("Could not acquire DB lock")?;
        db::collaborative_reverts::insert(
            &mut conn,
            position::models::CollaborativeRevert {
                channel_id,
                trader_pubkey: to_secp_pk_30(peer_id),
                coordinator_address: coordinator_address.clone(),
                coordinator_amount_sats: coordinator_amount,
                trader_amount_sats: trader_amount,
                timestamp: OffsetDateTime::now_utc(),
                price: closing_price,
            },
        )
        .context("Could not insert new collaborative revert")?
    };

    sender
        .send(OrderbookMessage::TraderMessage {
            trader_id: to_secp_pk_30(peer_id),
            message: Message::DlcChannelCollaborativeRevert {
                channel_id,
                coordinator_address: Address::new(
                    coordinator_address.network,
                    coordinator_address.payload,
                ),
                coordinator_amount,
                trader_amount,
                execution_price: closing_price,
            },
            notification: Some(NotificationKind::CollaborativeRevert),
        })
        .await
        .context("Failed to notify user")?;

    Ok(())
}

/// Complete the collaborative revert protocol by:
///
/// 1. Verifying the contents of the transaction sent by the counterparty.
/// 2. Signing the transaction.
/// 3. Broadcasting the signed transaction.
pub fn confirm_collaborative_revert(
    node: Arc<
        Node<
            bdk_file_store::Store<bdk::wallet::ChangeSet>,
            CoordinatorTenTenOneStorage,
            NodeStorage,
        >,
    >,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    channel_id: DlcChannelId,
    revert_transaction: Transaction,
    counterparty_signature: Signature,
) -> Result<Transaction> {
    let channel_id_hex = hex::encode(channel_id);

    let record = db::collaborative_reverts::get_by_channel_id(conn, &channel_id, node.network)?
        .with_context(|| {
            format!(
                "No matching record to confirm collaborative revert for channel {channel_id_hex}"
            )
        })?;

    tracing::info!(
        collaborative_revert_record = ?record,
        "Confirming collaborative revert"
    );

    // TODO: Check if provided amounts are as expected.

    let does_revert_pay_to_coordinator = revert_transaction
        .output
        .iter()
        .any(|output| node.is_mine(&output.script_pubkey));

    ensure!(
        does_revert_pay_to_coordinator,
        "Proposed collaborative revert transaction doesn't pay the coordinator"
    );

    let signed_channels = node
        .list_signed_dlc_channels()
        .context("Failed to list signed DLC channels")?;
    let signed_channel = signed_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("DLC channel to be reverted not found")?;

    let fund_out_amount = signed_channel.fund_tx.output[signed_channel.fund_output_index].value;

    let own_fund_sk = node
        .dlc_wallet
        .get_secret_key_for_pubkey(&signed_channel.own_params.fund_pubkey)?;

    let mut revert_transaction = to_tx_29(revert_transaction);
    dlc::util::sign_multi_sig_input(
        &bitcoin_old::secp256k1::Secp256k1::new(),
        &mut revert_transaction,
        &to_ecdsa_signature_29(counterparty_signature),
        &signed_channel.counter_params.fund_pubkey,
        &own_fund_sk,
        &signed_channel.fund_script_pubkey,
        fund_out_amount,
        0,
    )?;

    let revert_transaction = to_tx_30(revert_transaction);

    tracing::info!(
        txid = revert_transaction.txid().to_string(),
        "Broadcasting collaborative revert transaction"
    );

    block_on(node.blockchain.broadcast_transaction(&revert_transaction))
        .context("Could not broadcast transaction")?;

    // TODO: We should probably not modify the state until the transaction has been confirmed.

    let position = Position::get_position_by_trader(conn, record.trader_pubkey, vec![])?
        .with_context(|| format!("Could not load position for subchannel {channel_id_hex}"))?;

    Position::set_position_to_closed(conn, position.id)
        .context("Could not set position to closed")?;

    db::collaborative_reverts::delete(conn, channel_id)?;

    node.dlc_manager.get_store().upsert_channel(
        dlc_manager::channel::Channel::CollaborativelyClosed(ClosedChannel {
            counter_party: signed_channel.counter_party,
            temporary_channel_id: signed_channel.temporary_channel_id,
            channel_id: signed_channel.channel_id,
            reference_id: None,
            closing_txid: to_txid_29(revert_transaction.txid()),
        }),
        // The contract doesn't matter anymore.
        None,
    )?;

    Ok(revert_transaction)
}
