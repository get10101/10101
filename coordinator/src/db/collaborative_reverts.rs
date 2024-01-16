use crate::parse_dlc_channel_id;
use crate::position;
use crate::schema::collaborative_reverts;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Denomination;
use diesel::prelude::*;
use diesel::AsChangeset;
use diesel::Insertable;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::Queryable;
use diesel::RunQueryDsl;
use dlc_manager::DlcChannelId;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Queryable, AsChangeset, Debug, Clone, PartialEq)]
#[diesel(table_name = collaborative_reverts)]
pub(crate) struct CollaborativeRevert {
    id: i32,
    channel_id: String,
    trader_pubkey: String,
    price: f32,
    coordinator_address: String,
    coordinator_amount_sats: i64,
    trader_amount_sats: i64,
    timestamp: OffsetDateTime,
}

#[derive(Insertable, Queryable, AsChangeset, Debug, Clone, PartialEq)]
#[diesel(table_name = collaborative_reverts)]
pub(crate) struct NewCollaborativeRevert {
    channel_id: String,
    trader_pubkey: String,
    price: f32,
    coordinator_address: String,
    coordinator_amount_sats: i64,
    trader_amount_sats: i64,
    timestamp: OffsetDateTime,
}

pub(crate) fn by_trader_pubkey(
    trader_pubkey: &str,
    conn: &mut PgConnection,
) -> Result<Option<position::models::CollaborativeRevert>> {
    let result: Option<CollaborativeRevert> = collaborative_reverts::table
        .filter(collaborative_reverts::trader_pubkey.eq(trader_pubkey))
        .first(conn)
        .optional()?;
    if let Some(rev) = result {
        let rev = rev.try_into()?;
        Ok(Some(rev))
    } else {
        Ok(None)
    }
}

pub(crate) fn get_by_channel_id(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
) -> Result<Option<position::models::CollaborativeRevert>> {
    let channel_id = channel_id.to_hex();

    collaborative_reverts::table
        .filter(collaborative_reverts::channel_id.eq(channel_id))
        .first(conn)
        .optional()?
        .map(|rev: CollaborativeRevert| anyhow::Ok(rev.try_into()?))
        .transpose()
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    collaborative_reverts: position::models::CollaborativeRevert,
) -> Result<()> {
    let revert = NewCollaborativeRevert::from(collaborative_reverts);
    let affected_rows = diesel::insert_into(collaborative_reverts::table)
        .values(revert)
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert collaborative revert");

    Ok(())
}

pub(crate) fn delete(conn: &mut PgConnection, channel_id: DlcChannelId) -> Result<()> {
    diesel::delete(collaborative_reverts::table)
        .filter(collaborative_reverts::channel_id.eq(channel_id.to_hex()))
        .execute(conn)?;

    Ok(())
}

impl From<position::models::CollaborativeRevert> for NewCollaborativeRevert {
    fn from(value: position::models::CollaborativeRevert) -> Self {
        NewCollaborativeRevert {
            channel_id: value.channel_id.to_hex(),
            trader_pubkey: value.trader_pubkey.to_string(),
            price: value.price.to_f32().expect("to be valid f32"),
            coordinator_address: value.coordinator_address.to_string(),
            coordinator_amount_sats: value.coordinator_amount_sats.to_sat() as i64,
            trader_amount_sats: value.trader_amount_sats.to_sat() as i64,
            timestamp: value.timestamp,
        }
    }
}

impl TryFrom<CollaborativeRevert> for position::models::CollaborativeRevert {
    type Error = anyhow::Error;

    fn try_from(value: CollaborativeRevert) -> std::result::Result<Self, Self::Error> {
        Ok(position::models::CollaborativeRevert {
            channel_id: parse_dlc_channel_id(value.channel_id.as_str())?,
            trader_pubkey: PublicKey::from_str(value.trader_pubkey.as_str())?,
            price: Decimal::from_f32(value.price).expect("to be valid decimal"),
            coordinator_address: Address::from_str(value.coordinator_address.as_str())?,
            coordinator_amount_sats: Amount::from_str_in(
                value.coordinator_amount_sats.to_string().as_str(),
                Denomination::Satoshi,
            )?,
            trader_amount_sats: Amount::from_str_in(
                value.trader_amount_sats.to_string().as_str(),
                Denomination::Satoshi,
            )?,
            timestamp: value.timestamp,
        })
    }
}
