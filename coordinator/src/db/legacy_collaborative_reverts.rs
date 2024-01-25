use crate::parse_channel_id;
use crate::position;
use crate::schema::legacy_collaborative_reverts;
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
use lightning::ln::ChannelId;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Queryable, AsChangeset, Debug, Clone, PartialEq)]
#[diesel(table_name = legacy_collaborative_reverts)]
pub(crate) struct LegacyCollaborativeRevert {
    id: i32,
    channel_id: String,
    trader_pubkey: String,
    price: f32,
    coordinator_address: String,
    coordinator_amount_sats: i64,
    trader_amount_sats: i64,
    funding_txid: String,
    funding_vout: i32,
    timestamp: OffsetDateTime,
}

#[derive(Insertable, Queryable, AsChangeset, Debug, Clone, PartialEq)]
#[diesel(table_name = legacy_collaborative_reverts)]
pub(crate) struct NewLegacyCollaborativeRevert {
    channel_id: String,
    trader_pubkey: String,
    price: f32,
    coordinator_address: String,
    coordinator_amount_sats: i64,
    trader_amount_sats: i64,
    funding_txid: String,
    funding_vout: i32,
    timestamp: OffsetDateTime,
}

pub(crate) fn get_by_channel_id(
    conn: &mut PgConnection,
    channel_id: &ChannelId,
) -> Result<Option<position::models::LegacyCollaborativeRevert>> {
    let channel_id = channel_id.0.to_hex();

    legacy_collaborative_reverts::table
        .filter(legacy_collaborative_reverts::channel_id.eq(channel_id))
        .first(conn)
        .optional()?
        .map(|rev: LegacyCollaborativeRevert| anyhow::Ok(rev.try_into()?))
        .transpose()
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    collaborative_reverts: position::models::LegacyCollaborativeRevert,
) -> Result<()> {
    let revert = NewLegacyCollaborativeRevert::from(collaborative_reverts);
    let affected_rows = diesel::insert_into(legacy_collaborative_reverts::table)
        .values(revert)
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert collaborative revert");

    Ok(())
}

pub(crate) fn delete(conn: &mut PgConnection, channel_id: ChannelId) -> Result<()> {
    diesel::delete(legacy_collaborative_reverts::table)
        .filter(legacy_collaborative_reverts::channel_id.eq(channel_id.to_hex()))
        .execute(conn)?;

    Ok(())
}

impl From<position::models::LegacyCollaborativeRevert> for NewLegacyCollaborativeRevert {
    fn from(value: position::models::LegacyCollaborativeRevert) -> Self {
        NewLegacyCollaborativeRevert {
            channel_id: value.channel_id.0.to_hex(),
            trader_pubkey: value.trader_pubkey.to_string(),
            price: value.price,
            coordinator_address: value.coordinator_address.to_string(),
            coordinator_amount_sats: value.coordinator_amount_sats.to_sat() as i64,
            trader_amount_sats: value.trader_amount_sats.to_sat() as i64,
            funding_txid: value.txid.to_string(),
            funding_vout: value.vout as i32,
            timestamp: value.timestamp,
        }
    }
}

impl TryFrom<LegacyCollaborativeRevert> for position::models::LegacyCollaborativeRevert {
    type Error = anyhow::Error;

    fn try_from(value: LegacyCollaborativeRevert) -> std::result::Result<Self, Self::Error> {
        Ok(position::models::LegacyCollaborativeRevert {
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
            txid: Txid::from_str(&value.funding_txid).context("To have valid txid")?,
            vout: value.funding_vout as u32,
            timestamp: value.timestamp,
        })
    }
}
