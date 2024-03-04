use crate::config;
use crate::db;
use crate::db::connection;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::state::get_node;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::SecretKey;
use dlc_manager::DlcChannelId;
use dlc_manager::Signer;
use dlc_messages::channel::SettleFinalize;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use hex::FromHex;
use lightning::ln::chan_utils::build_commitment_secret;
use ln_dlc_node::bitcoin_conversion::to_secp_sk_29;
use ln_dlc_node::node::event::NodeEvent;
use trade::ContractSymbol;

pub fn set_filling_orders_to_failed() -> Result<()> {
    tracing::warn!("Executing emergency kit! Setting orders in state Filling to Failed!");

    let mut conn = connection()?;
    db::models::Order::set_all_filling_orders_to_failed(&mut conn)
}

pub fn delete_dlc_channel(dlc_channel_id: String) -> Result<()> {
    tracing::warn!(
        dlc_channel_id,
        "Executing emergency kit! Deleting dlc channel"
    );
    let dlc_channel_id = DlcChannelId::from_hex(dlc_channel_id)?;
    ln_dlc::delete_dlc_channel(&dlc_channel_id)
}

pub fn delete_position() -> Result<()> {
    tracing::warn!("Executing emergency kit! Deleting position!");
    db::delete_positions()?;
    event::publish(&EventInternal::PositionCloseNotification(
        ContractSymbol::BtcUsd,
    ));
    Ok(())
}

pub fn resend_settle_finalize_message() -> Result<()> {
    tracing::warn!("Executing emergency kit! Resending settle finalize message");
    let coordinator_pubkey = config::get_coordinator_info().pubkey;

    let node = get_node();
    let signed_channel = node
        .inner
        .get_signed_channel_by_trader_id(coordinator_pubkey)?;

    ensure!(
        matches!(
            signed_channel.state,
            dlc_manager::channel::signed_channel::SignedChannelState::Settled { .. }
        ),
        "Signed channel state must be settled to resend settle finalize message!"
    );

    let per_update_seed_pk = signed_channel.own_per_update_seed;
    let per_update_seed = node
        .inner
        .dlc_wallet
        .get_secret_key_for_pubkey(&per_update_seed_pk)?;
    let prev_per_update_secret = SecretKey::from_slice(&build_commitment_secret(
        per_update_seed.as_ref(),
        signed_channel.update_idx + 1,
    ))?;

    let msg = Message::Channel(ChannelMessage::SettleFinalize(SettleFinalize {
        channel_id: signed_channel.channel_id,
        prev_per_update_secret: to_secp_sk_29(prev_per_update_secret),
        reference_id: signed_channel.reference_id,
    }));

    node.inner
        .event_handler
        .publish(NodeEvent::SendDlcMessage {
            peer: coordinator_pubkey,
            msg: msg.clone(),
        })?;

    Ok(())
}
