use crate::position;
use crate::position::models::parse_channel_id;
use crate::schema::collaborative_reverts;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Denomination;
use bitcoin::Txid;
use diesel::prelude::*;
use diesel::AsChangeset;
use diesel::Insertable;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::Queryable;
use diesel::RunQueryDsl;
use dlc_manager::ChannelId;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Queryable, AsChangeset, Debug, Clone, PartialEq)]
#[diesel(table_name = collaborative_reverts)]
pub(crate) struct CollaborativeRevert {
    pub id: i32,
    pub channel_id: String,
    pub trader_pubkey: String,
    pub price: f32,
    pub coordinator_address: String,
    pub coordinator_amount_sats: i64,
    pub trader_amount_sats: i64,
    pub timestamp: OffsetDateTime,
    pub funding_txid: String,
    pub funding_vout: i32,
}

#[derive(Insertable, Queryable, AsChangeset, Debug, Clone, PartialEq)]
#[diesel(table_name = collaborative_reverts)]
pub(crate) struct NewCollaborativeRevert {
    pub channel_id: String,
    pub trader_pubkey: String,
    pub price: f32,
    pub coordinator_address: String,
    pub coordinator_amount_sats: i64,
    pub trader_amount_sats: i64,
    pub timestamp: OffsetDateTime,
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

pub(crate) fn delete(conn: &mut PgConnection, channel_id: ChannelId) -> Result<()> {
    diesel::delete(collaborative_reverts::table)
        .filter(collaborative_reverts::channel_id.eq(channel_id.to_hex()))
        .execute(conn)?;

    Ok(())
}

impl From<position::models::CollaborativeRevert> for NewCollaborativeRevert {
    fn from(value: position::models::CollaborativeRevert) -> Self {
        NewCollaborativeRevert {
            channel_id: hex::encode(value.channel_id),
            trader_pubkey: value.trader_pubkey.to_string(),
            price: value.price,
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
            channel_id: parse_channel_id(value.channel_id.as_str())?,
            trader_pubkey: PublicKey::from_str(value.trader_pubkey.as_str())?,
            price: value.price,
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
            txid: Txid::from_str(&value.funding_txid).context("To have valid txid")?,
            vout: value.funding_vout as u32,
        })
    }
}
