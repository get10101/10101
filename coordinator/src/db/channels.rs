use crate::schema;
use crate::schema::channels;
use crate::schema::sql_types::ChannelStateType;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Txid;
use diesel::query_builder::QueryId;
use diesel::AsChangeset;
use diesel::AsExpression;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::FromSqlRow;
use diesel::Insertable;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use dlc_manager::ChannelId;
use hex::FromHex;
use lightning::ln::PaymentHash;
use ln_dlc_node::channel::UserChannelId;
use std::any::TypeId;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = ChannelStateType)]
pub(crate) enum ChannelState {
    Announced,
    Pending,
    Open,
    Closed,
    ForceClosedRemote,
    ForceClosedLocal,
}

impl QueryId for ChannelStateType {
    type QueryId = ChannelStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

#[derive(Insertable, QueryableByName, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = channels)]
pub(crate) struct Channel {
    pub user_channel_id: String,
    pub channel_id: Option<String>,
    pub inbound_sats: i64,
    pub outbound_sats: i64,
    pub funding_txid: Option<String>,
    pub channel_state: ChannelState,
    pub counterparty_pubkey: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub open_channel_fee_payment_hash: Option<String>,
    pub liquidity_option_id: Option<i32>,
}

pub(crate) fn get(user_channel_id: &str, conn: &mut PgConnection) -> QueryResult<Option<Channel>> {
    channels::table
        .filter(channels::user_channel_id.eq(user_channel_id))
        .first(conn)
        .optional()
}

pub(crate) fn get_announced_channel(
    counterparty_pubkey: &str,
    conn: &mut PgConnection,
) -> QueryResult<Option<Channel>> {
    channels::table
        .filter(channels::counterparty_pubkey.eq(counterparty_pubkey))
        .filter(channels::channel_state.eq(ChannelState::Announced))
        .first(conn)
        .optional()
}

pub(crate) fn get_all_non_pending_channels(conn: &mut PgConnection) -> QueryResult<Vec<Channel>> {
    channels::table
        .filter(
            channels::channel_state
                .ne(ChannelState::Pending)
                .and(schema::channels::funding_txid.is_not_null()),
        )
        .load(conn)
}

pub(crate) fn update_payment_hash(
    payment_hash: PaymentHash,
    funding_txid: String,
    conn: &mut PgConnection,
) -> Result<()> {
    let mut channel: Channel = channels::table
        .filter(channels::funding_txid.eq(funding_txid.clone()))
        .first(conn)
        .with_context(|| format!("No channel found for funding txid {funding_txid}"))?;

    channel.open_channel_fee_payment_hash = Some(payment_hash.0.to_hex());
    upsert(channel, conn)
}

pub fn get_by_channel_id(
    channel_id: String,
    conn: &mut PgConnection,
) -> Result<Option<ln_dlc_node::channel::Channel>> {
    let channel = channels::table
        .filter(channels::channel_id.eq(channel_id))
        .first::<Channel>(conn)
        .optional()?
        .map(ln_dlc_node::channel::Channel::from);
    Ok(channel)
}

pub(crate) fn upsert(channel: Channel, conn: &mut PgConnection) -> Result<()> {
    let affected_rows = diesel::insert_into(channels::table)
        .values(channel.clone())
        .on_conflict(schema::channels::user_channel_id)
        .do_update()
        .set(&channel)
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not upsert channel");

    Ok(())
}

impl From<ln_dlc_node::channel::Channel> for Channel {
    fn from(value: ln_dlc_node::channel::Channel) -> Self {
        Channel {
            user_channel_id: value.user_channel_id.to_string(),
            channel_id: value.channel_id.map(|cid| cid.to_hex()),
            liquidity_option_id: value.liquidity_option_id,
            inbound_sats: value.inbound_sats as i64,
            outbound_sats: value.outbound_sats as i64,
            funding_txid: value.funding_txid.map(|txid| txid.to_string()),
            channel_state: value.channel_state.into(),
            counterparty_pubkey: value.counterparty.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            open_channel_fee_payment_hash: None,
        }
    }
}

impl From<ln_dlc_node::channel::ChannelState> for ChannelState {
    fn from(value: ln_dlc_node::channel::ChannelState) -> Self {
        match value {
            ln_dlc_node::channel::ChannelState::Announced => ChannelState::Announced,
            ln_dlc_node::channel::ChannelState::Pending => ChannelState::Pending,
            ln_dlc_node::channel::ChannelState::Open => ChannelState::Open,
            // mapping `ChannelState::OpenUnpaid` to open as the coordinator references the payment
            // through the payment hash. Hence no need to reflect that state twice.
            ln_dlc_node::channel::ChannelState::OpenUnpaid => ChannelState::Open,
            ln_dlc_node::channel::ChannelState::Closed => ChannelState::Closed,
            ln_dlc_node::channel::ChannelState::ForceClosedLocal => ChannelState::ForceClosedLocal,
            ln_dlc_node::channel::ChannelState::ForceClosedRemote => {
                ChannelState::ForceClosedRemote
            }
        }
    }
}

impl From<Channel> for ln_dlc_node::channel::Channel {
    fn from(value: Channel) -> Self {
        ln_dlc_node::channel::Channel {
            user_channel_id: UserChannelId::try_from(value.user_channel_id)
                .expect("valid user channel id"),
            channel_id: value
                .channel_id
                .map(|cid| ChannelId::from_hex(cid).expect("valid channel id")),
            liquidity_option_id: value.liquidity_option_id,
            inbound_sats: value.inbound_sats as u64,
            outbound_sats: value.outbound_sats as u64,
            funding_txid: value
                .funding_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            channel_state: value.channel_state.into(),
            counterparty: PublicKey::from_str(&value.counterparty_pubkey)
                .expect("valid public key"),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<ChannelState> for ln_dlc_node::channel::ChannelState {
    fn from(value: ChannelState) -> Self {
        match value {
            ChannelState::Announced => ln_dlc_node::channel::ChannelState::Announced,
            ChannelState::Pending => ln_dlc_node::channel::ChannelState::Pending,
            ChannelState::Open => ln_dlc_node::channel::ChannelState::Open,
            ChannelState::Closed => ln_dlc_node::channel::ChannelState::Closed,
            ChannelState::ForceClosedLocal => ln_dlc_node::channel::ChannelState::ForceClosedLocal,
            ChannelState::ForceClosedRemote => {
                ln_dlc_node::channel::ChannelState::ForceClosedRemote
            }
        }
    }
}
