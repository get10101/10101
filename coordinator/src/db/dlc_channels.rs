use crate::dlc_protocol::ProtocolId;
use crate::node::channel;
use crate::schema::dlc_channels;
use crate::schema::sql_types::DlcChannelStateType;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::Txid;
use bitcoin_old::hashes::hex::ToHex;
use diesel::query_builder::QueryId;
use diesel::AsChangeset;
use diesel::AsExpression;
use diesel::ExpressionMethods;
use diesel::FromSqlRow;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use dlc_manager::DlcChannelId;
use hex::FromHex;
use std::any::TypeId;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = DlcChannelStateType)]
pub(crate) enum DlcChannelState {
    Pending,
    Open,
    Closing,
    Closed,
    Failed,
    Cancelled,
}

#[derive(QueryableByName, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = dlc_channels)]
pub(crate) struct DlcChannel {
    id: i32,
    open_protocol_id: Uuid,
    channel_id: String,
    trader_pubkey: String,
    channel_state: DlcChannelState,
    trader_reserve_sats: i64,
    coordinator_reserve_sats: i64,
    funding_txid: Option<String>,
    close_txid: Option<String>,
    settle_txid: Option<String>,
    buffer_txid: Option<String>,
    claim_txid: Option<String>,
    punish_txid: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    coordinator_funding_sats: i64,
    trader_funding_sats: i64,
}

impl QueryId for DlcChannelStateType {
    type QueryId = DlcChannelStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

pub(crate) fn insert_pending_dlc_channel(
    conn: &mut PgConnection,
    open_protocol_id: &ProtocolId,
    channel_id: &DlcChannelId,
    trader: &PublicKey,
) -> QueryResult<usize> {
    diesel::insert_into(dlc_channels::table)
        .values((
            dlc_channels::open_protocol_id.eq(open_protocol_id.to_uuid()),
            dlc_channels::channel_id.eq(channel_id.to_hex()),
            dlc_channels::channel_state.eq(DlcChannelState::Pending),
            dlc_channels::coordinator_reserve_sats.eq(0),
            dlc_channels::trader_reserve_sats.eq(0),
            dlc_channels::trader_pubkey.eq(trader.to_string()),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
            dlc_channels::created_at.eq(OffsetDateTime::now_utc()),
        ))
        .execute(conn)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn set_dlc_channel_open(
    conn: &mut PgConnection,
    open_protocol_id: &ProtocolId,
    channel_id: &DlcChannelId,
    funding_txid: Txid,
    coordinator_reserve: Amount,
    trader_reserve: Amount,
    coordinator_funding: Amount,
    trader_funding: Amount,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::funding_txid.eq(funding_txid.to_string()),
            dlc_channels::channel_id.eq(channel_id.to_hex()),
            dlc_channels::channel_state.eq(DlcChannelState::Open),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
            dlc_channels::coordinator_reserve_sats.eq(coordinator_reserve.to_sat() as i64),
            dlc_channels::trader_reserve_sats.eq(trader_reserve.to_sat() as i64),
            dlc_channels::coordinator_funding_sats.eq(coordinator_funding.to_sat() as i64),
            dlc_channels::trader_funding_sats.eq(trader_funding.to_sat() as i64),
        ))
        .filter(dlc_channels::open_protocol_id.eq(open_protocol_id.to_uuid()))
        .execute(conn)
}

pub(crate) fn update_channel(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
    coordinator_reserve: Amount,
    trader_reserve: Amount,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
            dlc_channels::coordinator_reserve_sats.eq(coordinator_reserve.to_sat() as i64),
            dlc_channels::trader_reserve_sats.eq(trader_reserve.to_sat() as i64),
        ))
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .execute(conn)
}

pub(crate) fn set_channel_force_closing_settled(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
    settle_txid: Txid,
    claim_txid: Option<Txid>,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::settle_txid.eq(settle_txid.to_string()),
            dlc_channels::claim_txid.eq(claim_txid.map(|txid| txid.to_string())),
            dlc_channels::channel_state.eq(DlcChannelState::Closing),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .execute(conn)
}

pub(crate) fn set_channel_force_closing(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
    buffer_txid: Txid,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::buffer_txid.eq(buffer_txid.to_string()),
            dlc_channels::channel_state.eq(DlcChannelState::Closing),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .execute(conn)
}

pub(crate) fn set_channel_punished(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
    punish_txid: Txid,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::punish_txid.eq(punish_txid.to_string()),
            dlc_channels::channel_state.eq(DlcChannelState::Closing),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .execute(conn)
}

pub(crate) fn set_channel_collab_closing(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
    close_txid: Txid,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::close_txid.eq(close_txid.to_string()),
            dlc_channels::channel_state.eq(DlcChannelState::Closing),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .execute(conn)
}

pub(crate) fn set_channel_collab_closed(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
    close_txid: Txid,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::close_txid.eq(close_txid.to_string()),
            dlc_channels::channel_state.eq(DlcChannelState::Closed),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .execute(conn)
}

pub(crate) fn set_channel_failed(
    conn: &mut PgConnection,
    protocol_id: &ProtocolId,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::channel_state.eq(DlcChannelState::Failed),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::open_protocol_id.eq(protocol_id.to_uuid()))
        .execute(conn)
}

pub(crate) fn set_channel_cancelled(
    conn: &mut PgConnection,
    protocol_id: &ProtocolId,
) -> QueryResult<usize> {
    diesel::update(dlc_channels::table)
        .set((
            dlc_channels::channel_state.eq(DlcChannelState::Cancelled),
            dlc_channels::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .filter(dlc_channels::open_protocol_id.eq(protocol_id.to_uuid()))
        .execute(conn)
}

pub(crate) fn get_dlc_channel(
    conn: &mut PgConnection,
    channel_id: &DlcChannelId,
) -> QueryResult<Option<channel::DlcChannel>> {
    let dlc_channel: Option<DlcChannel> = dlc_channels::table
        .filter(dlc_channels::channel_id.eq(channel_id.to_hex()))
        .first(conn)
        .optional()?;

    Ok(dlc_channel.map(channel::DlcChannel::from))
}

impl From<DlcChannel> for channel::DlcChannel {
    fn from(value: DlcChannel) -> Self {
        Self {
            channel_id: DlcChannelId::from_hex(value.channel_id).expect("valid dlc channel id"),
            trader: PublicKey::from_str(&value.trader_pubkey).expect("valid pubkey"),
            channel_state: channel::DlcChannelState::from(value.channel_state),
            trader_reserve_sats: Amount::from_sat(value.trader_reserve_sats as u64),
            coordinator_reserve_sats: Amount::from_sat(value.coordinator_reserve_sats as u64),
            trader_funding_sats: Amount::from_sat(value.trader_funding_sats as u64),
            coordinator_funding_sats: Amount::from_sat(value.coordinator_funding_sats as u64),
            funding_txid: value
                .funding_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            close_txid: value
                .close_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            settle_txid: value
                .settle_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            buffer_txid: value
                .buffer_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            claim_txid: value
                .claim_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            punish_txid: value
                .punish_txid
                .map(|txid| Txid::from_str(&txid).expect("valid txid")),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<DlcChannelState> for channel::DlcChannelState {
    fn from(value: DlcChannelState) -> Self {
        match value {
            DlcChannelState::Pending => channel::DlcChannelState::Pending,
            DlcChannelState::Open => channel::DlcChannelState::Open,
            DlcChannelState::Closing => channel::DlcChannelState::Closing,
            DlcChannelState::Closed => channel::DlcChannelState::Closed,
            DlcChannelState::Failed => channel::DlcChannelState::Failed,
            DlcChannelState::Cancelled => channel::DlcChannelState::Cancelled,
        }
    }
}
