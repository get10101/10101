use crate::schema;
use crate::schema::channels;
use crate::schema::sql_types::ChannelStateType;
use anyhow::ensure;
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
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use dlc_manager::ChannelId;
use hex::FromHex;
use ln_dlc_node::channel::UserChannelId;
use std::any::TypeId;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = ChannelStateType)]
pub(crate) enum ChannelState {
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
    pub capacity: i64,
    pub balance: i64,
    pub funding_txid: Option<String>,
    pub channel_state: ChannelState,
    pub counterparty_pubkey: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub costs: i64,
}

pub(crate) fn get(user_channel_id: &str, conn: &mut PgConnection) -> QueryResult<Channel> {
    channels::table
        .filter(channels::user_channel_id.eq(user_channel_id))
        .first(conn)
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

pub(crate) fn upsert(channel: Channel, conn: &mut PgConnection) -> Result<()> {
    let affected_rows = diesel::insert_into(channels::table)
        .values(channel.clone())
        .on_conflict(schema::channels::user_channel_id)
        .do_update()
        .set(&channel)
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert channel");

    Ok(())
}

impl From<ln_dlc_node::channel::Channel> for Channel {
    fn from(value: ln_dlc_node::channel::Channel) -> Self {
        Channel {
            user_channel_id: value.user_channel_id.to_string(),
            channel_id: value.channel_id.map(|cid| cid.to_hex()),
            capacity: value.capacity as i64,
            balance: value.balance as i64,
            funding_txid: value.funding_txid.map(|txid| txid.to_string()),
            channel_state: value.channel_state.into(),
            counterparty_pubkey: value.counterparty.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            costs: value.costs as i64,
        }
    }
}

impl From<ln_dlc_node::channel::ChannelState> for ChannelState {
    fn from(value: ln_dlc_node::channel::ChannelState) -> Self {
        match value {
            ln_dlc_node::channel::ChannelState::Pending => ChannelState::Pending,
            ln_dlc_node::channel::ChannelState::Open => ChannelState::Open,
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
            capacity: value.capacity as u64,
            balance: value.balance as u64,
            funding_txid: value
                .funding_txid
                .map(|txid| Txid::from_str(&txid).expect("valid funding txid")),
            channel_state: value.channel_state.into(),
            counterparty: PublicKey::from_str(&value.counterparty_pubkey)
                .expect("valid public key"),
            created_at: value.created_at,
            updated_at: value.updated_at,
            costs: value.costs as u64,
        }
    }
}

impl From<ChannelState> for ln_dlc_node::channel::ChannelState {
    fn from(value: ChannelState) -> Self {
        match value {
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
