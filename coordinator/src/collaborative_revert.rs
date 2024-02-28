use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::notifications::NotificationKind;
use crate::position;
use crate::position::models::LegacyCollaborativeRevert;
use crate::storage::CoordinatorTenTenOneStorage;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::Transaction;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc::util::weight_to_fee;
use dlc_manager::channel::ClosedChannel;
use dlc_manager::subchannel::LNChannelManager;
use dlc_manager::subchannel::LnDlcChannelSigner;
use dlc_manager::subchannel::LnDlcSignerProvider;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::DlcChannelId;
use dlc_manager::Signer;
use dlc_manager::Storage;
use lightning::ln::ChannelId;
use ln_dlc_node::node::Node;
use rust_decimal::prelude::ToPrimitive;
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
    node: Arc<Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    pool: Pool<ConnectionManager<PgConnection>>,
    sender: mpsc::Sender<OrderbookMessage>,
    channel_id: DlcChannelId,
    fee_rate_sats_vb: u64,
    trader_amount_sats: u64,
    closing_price: Decimal,
) -> Result<()> {
    let channel_id_hex = channel_id.to_hex();

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

    let fee = weight_to_fee(COLLABORATIVE_REVERT_TX_WEIGHT, fee_rate_sats_vb)
        .context("Could not calculate fee")?;

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

    {
        let mut conn = pool.get().context("Could not acquire DB lock")?;
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
        .context("Could not insert new collaborative revert")?
    };

    sender
        .send(OrderbookMessage::TraderMessage {
            trader_id: peer_id,
            message: Message::DlcChannelCollaborativeRevert {
                channel_id,
                coordinator_address,
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
    node: Arc<Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    channel_id: DlcChannelId,
    mut revert_transaction: Transaction,
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

    let signed_channels = node
        .list_signed_dlc_channels()
        .context("Failed to list signed DLC channels")?;
    let signed_channel = signed_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("DLC channel to be reverted not found")?;

    let fund_out_amount = signed_channel.fund_tx.output[signed_channel.fund_output_index].value;

    let own_fund_sk = node
        .wallet()
        .get_secret_key_for_pubkey(&signed_channel.own_params.fund_pubkey)?;

    dlc::util::sign_multi_sig_input(
        &Secp256k1::new(),
        &mut revert_transaction,
        &counterparty_signature,
        &signed_channel.counter_params.fund_pubkey,
        &own_fund_sk,
        &signed_channel.fund_script_pubkey,
        fund_out_amount,
        0,
    )?;

    tracing::info!(
        txid = revert_transaction.txid().to_string(),
        "Broadcasting collaborative revert transaction"
    );

    node.ldk_wallet()
        .broadcast_transaction(&revert_transaction)
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
        }),
        // The contract doesn't matter anymore.
        None,
    )?;

    Ok(revert_transaction)
}

/// Propose collaboratively reverting the _LN-DLC_ channel identified by `channel_id`, without LDK's
/// [`ChannelDetails`] for said channel.
///
/// A collaborative revert involves signing a new transaction spending from the funding output
/// directly. This can be used to circumvent bugs related to position and subchannel state.
#[allow(clippy::too_many_arguments)]
pub async fn propose_legacy_collaborative_revert(
    node: Arc<Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    pool: Pool<ConnectionManager<PgConnection>>,
    sender: mpsc::Sender<OrderbookMessage>,
    channel_id: ChannelId,
    funding_txo: OutPoint,
    coordinator_amount: u64,
    fee_rate_sats_vb: u64,
    // The settlement price is purely informational for the counterparty.
    settlement_price: Decimal,
) -> Result<()> {
    let mut conn = pool.get().context("Could not acquire DB lock")?;

    let channel_id_hex = channel_id.to_hex();

    let subchannels = node
        .list_sub_channels()
        .context("Could not get list of subchannels")?;

    let subchannel = subchannels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Missing subchannel")?;

    let peer_id = subchannel.counter_party;

    let trader_amount = subchannel.fund_value_satoshis - coordinator_amount;

    let fee = weight_to_fee(COLLABORATIVE_REVERT_TX_WEIGHT, fee_rate_sats_vb)
        .expect("To be able to calculate constant fee rate");

    let coordinator_address = node.get_unused_address();
    let coordinator_amount = Amount::from_sat(coordinator_amount - fee / 2);
    let trader_amount = Amount::from_sat(trader_amount - fee / 2);

    tracing::info!(
        channel_id = channel_id_hex,
        coordinator_address = %coordinator_address,
        coordinator_amount = coordinator_amount.to_sat(),
        trader_amount = trader_amount.to_sat(),
        "Proposing legacy collaborative revert"
    );

    db::legacy_collaborative_reverts::insert(
        &mut conn,
        LegacyCollaborativeRevert {
            channel_id,
            trader_pubkey: peer_id,
            price: settlement_price.to_f32().expect("to fit into f32"),
            coordinator_address: coordinator_address.clone(),
            coordinator_amount_sats: coordinator_amount,
            trader_amount_sats: trader_amount,
            timestamp: OffsetDateTime::now_utc(),
            txid: funding_txo.txid,
            vout: funding_txo.vout,
        },
    )
    .context("Could not insert new legacy collaborative revert")?;

    // Send collaborative revert proposal to the counterpary.
    sender
        .send(OrderbookMessage::TraderMessage {
            trader_id: peer_id,
            message: Message::CollaborativeRevert {
                channel_id: channel_id.0,
                coordinator_address,
                coordinator_amount,
                trader_amount,
                execution_price: settlement_price,
                funding_txo,
            },
            notification: Some(NotificationKind::CollaborativeRevert),
        })
        .await
        .context("Failed to notify user")?;

    Ok(())
}

/// Complete the collaborative revert protocol for _LN-DLC_ channels by:
///
/// 1. Verifying the contents of the transaction sent by the counterparty.
/// 2. Signing the transaction.
/// 3. Broadcasting the signed transaction.
pub fn confirm_legacy_collaborative_revert(
    node: Arc<Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    channel_id: ChannelId,
    mut revert_transaction: Transaction,
    counterparty_signature: Signature,
) -> Result<Transaction> {
    let channel_id_hex = channel_id.to_hex();

    let record = db::legacy_collaborative_reverts::get_by_channel_id(conn, &channel_id)?
        .with_context(|| {
            format!(
                "No matching record to confirm legacy collaborative revert for channel {channel_id_hex}"
            )
        })?;

    let funding_txid = &revert_transaction.input[0].previous_output.txid;
    let funding_tx = node
        .wallet()
        .get_transaction(funding_txid)
        .with_context(|| format!("Could not find funding transaction {funding_txid} on-chain"))?;

    tracing::info!(
        legacy_collaborative_revert_record = ?record,
        "Confirming legacy collaborative revert"
    );

    let does_revert_pay_to_coordinator = revert_transaction.output.iter().any(|output| {
        node.wallet()
            .is_mine(&output.script_pubkey)
            .unwrap_or_else(|e| {
                tracing::error!(
                    "Failed to confirm if proposed legacy collaborative revert \
                     transaction pays to the coordinator: {e:#}"
                );
                false
            })
    });

    ensure!(
        does_revert_pay_to_coordinator,
        "Proposed legacy collaborative revert transaction doesn't pay the coordinator"
    );

    let funding_txo = &funding_tx.output[record.vout as usize];

    let subchannels = node
        .list_sub_channels()
        .context("Failed to list subchannels")?;
    let optional_subchannel = subchannels.iter().find(|c| c.channel_id == channel_id);

    let (own_fund_pk, counter_fund_pk, funding_redeemscript) = {
        match optional_subchannel {
            Some(subchannel) => (
                subchannel.own_fund_pk,
                subchannel.counter_fund_pk,
                subchannel.original_funding_redeemscript.clone(),
            ),
            None => {
                let details = node
                    .channel_manager
                    .get_channel_details(&channel_id)
                    .with_context(|| {
                        format!("Could not get channel details for subchannel {channel_id_hex}")
                    })?;

                let counterparty_funding_pubkey = details.counter_funding_pubkey.context(
                    "Cannot confirm legacy collaborative revert without counterparty funding \
                     pubkey",
                )?;
                let funding_redeemscript = details.funding_redeemscript.context(
                    "Cannot confirm legacy collaborative revert without funding redeemscript",
                )?;
                (
                    details.holder_funding_pubkey,
                    counterparty_funding_pubkey,
                    funding_redeemscript,
                )
            }
        }
    };

    let channel_keys_id = match optional_subchannel.and_then(|sc| sc.channel_keys_id) {
        Some(channel_keys_id) => channel_keys_id,
        None => node
            .channel_manager
            .get_channel_details(&channel_id)
            .map(|c| c.channel_keys_id)
            .with_context(|| {
                format!("Could not get channel keys ID for subchannel {channel_id_hex}")
            })?,
    };

    let own_sig = {
        let fund_value_satoshis = funding_txo.value;

        let signer = node
            .keys_manager
            .derive_ln_dlc_channel_signer(fund_value_satoshis, channel_keys_id);

        signer
            .get_holder_split_tx_signature(
                &Secp256k1::new(),
                &revert_transaction,
                &funding_redeemscript,
                fund_value_satoshis,
            )
            .context("Could not get own signature for legacy collaborative revert transaction")?
    };

    let position = Position::get_position_by_trader(conn, record.trader_pubkey, vec![])?
        .with_context(|| format!("Could not load position for subchannel {channel_id_hex}"))?;

    dlc::util::finalize_multi_sig_input_transaction(
        &mut revert_transaction,
        vec![
            (own_fund_pk, own_sig),
            (counter_fund_pk, counterparty_signature),
        ],
        &funding_redeemscript,
        0,
    );

    tracing::info!(
        txid = revert_transaction.txid().to_string(),
        "Broadcasting legacy collaborative revert transaction"
    );
    node.ldk_wallet()
        .broadcast_transaction(&revert_transaction)
        .context("Could not broadcast transaction")?;

    // TODO: We should probably not modify the state until the transaction has been confirmed.

    Position::set_position_to_closed(conn, position.id)
        .context("Could not set position to closed")?;

    if let Some(mut subchannel) = optional_subchannel.cloned() {
        subchannel.state = SubChannelState::OnChainClosed;
        node.dlc_manager
            .get_store()
            .upsert_sub_channel(&subchannel)?;
    }

    db::legacy_collaborative_reverts::delete(conn, channel_id)?;

    Ok(revert_transaction)
}
