use crate::db;
use crate::db::connection;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use anyhow::Result;
use dlc_manager::DlcChannelId;
use hex::FromHex;
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
