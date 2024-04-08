use crate::calculations::calculate_liquidation_price;
use crate::config;
use crate::db;
use crate::db::connection;
use crate::event;
use crate::event::EventInternal;
use crate::get_maintenance_margin_rate;
use crate::ln_dlc;
use crate::state::get_node;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::SecretKey;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::contract::Contract;
use dlc_manager::DlcChannelId;
use dlc_manager::Signer;
use dlc_messages::channel::SettleFinalize;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use hex::FromHex;
use lightning::ln::chan_utils::build_commitment_secret;
use ln_dlc_node::bitcoin_conversion::to_secp_sk_29;
use ln_dlc_node::node::event::NodeEvent;
use time::OffsetDateTime;
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

pub fn recreate_position() -> Result<()> {
    tracing::warn!("Executing emergency kit! Recreating position!");
    let node = get_node();
    let counterparty = config::get_coordinator_info().pubkey;
    let channel = node.inner.get_signed_channel_by_trader_id(counterparty)?;

    ensure!(
        matches!(channel.state, SignedChannelState::Established { .. }),
        "A position can only be recreated from an established signed channel state"
    );

    let positions = db::get_positions()?;
    let position = positions.first();
    ensure!(
        position.is_none(),
        "Can't recreate a position if there is already a position"
    );

    let order = db::get_last_failed_order()?.context("Couldn't find last failed order!")?;
    let average_entry_price = order.execution_price().context("Missing execution price")?;

    tracing::debug!("Creating position from established signed dlc channel and last failed order");

    let contract_id = channel.get_contract_id().context("Missing contract id")?;

    let contract = node
        .inner
        .get_contract_by_id(&contract_id)?
        .context("Missing contract")?;

    let (collateral, expiry) = match contract {
        Contract::Signed(contract) | Contract::Confirmed(contract) => {
            let trader_reserve = node
                .inner
                .get_dlc_channel_usable_balance(&channel.channel_id)?;

            let oracle_event = &contract
                .accepted_contract
                .offered_contract
                .contract_info
                .first()
                .context("missing contract info")?
                .oracle_announcements
                .first()
                .context("missing oracle info")?
                .oracle_event;

            let expiry_timestamp =
                OffsetDateTime::from_unix_timestamp(oracle_event.event_maturity_epoch as i64)?;

            (
                contract.accepted_contract.accept_params.collateral - trader_reserve.to_sat(),
                expiry_timestamp,
            )
        }
        _ => {
            bail!("Contract in unexpected state: {:?}", contract);
        }
    };

    let maintenance_margin_rate = get_maintenance_margin_rate();
    let liquidation_price = calculate_liquidation_price(
        average_entry_price,
        order.leverage,
        order.direction,
        maintenance_margin_rate,
    );

    let position = Position {
        leverage: order.leverage,
        quantity: order.quantity,
        contract_symbol: order.contract_symbol,
        direction: order.direction,
        average_entry_price,
        liquidation_price,
        position_state: PositionState::Open,
        collateral,
        expiry,
        updated: OffsetDateTime::now_utc(),
        created: OffsetDateTime::now_utc(),
        stable: false,
    };
    db::insert_position(position.clone())?;

    event::publish(&EventInternal::PositionUpdateNotification(position));

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

    node.inner.event_handler.publish(NodeEvent::SendDlcMessage {
        peer: coordinator_pubkey,
        msg: msg.clone(),
    });

    Ok(())
}
