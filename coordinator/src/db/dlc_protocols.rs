use crate::db;
use crate::dlc_protocol;
use crate::dlc_protocol::ProtocolId;
use crate::schema::dlc_protocols;
use crate::schema::sql_types::ProtocolStateType;
use crate::schema::sql_types::ProtocolTypeType;
use bitcoin::secp256k1::PublicKey;
use diesel::query_builder::QueryId;
use diesel::AsExpression;
use diesel::ExpressionMethods;
use diesel::FromSqlRow;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use dlc_manager::ContractId;
use dlc_manager::DlcChannelId;
use hex::FromHex;
use std::any::TypeId;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression, Eq, Hash)]
#[diesel(sql_type = ProtocolStateType)]
pub(crate) enum DlcProtocolState {
    Pending,
    Success,
    Failed,
}

impl QueryId for ProtocolStateType {
    type QueryId = ProtocolStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression, Eq, Hash)]
#[diesel(sql_type = ProtocolTypeType)]
pub(crate) enum DlcProtocolType {
    OpenChannel,
    OpenPosition,
    Settle,
    Close,
    ForceClose,
    Rollover,
    ResizePosition,
}

impl QueryId for ProtocolTypeType {
    type QueryId = ProtocolTypeType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

#[derive(Queryable, Debug)]
#[diesel(table_name = protocols)]
#[allow(dead_code)] // We have to allow dead code here because diesel needs the fields to be able to derive queryable.
pub(crate) struct DlcProtocol {
    pub id: i32,
    pub protocol_id: Uuid,
    pub previous_protocol_id: Option<Uuid>,
    pub channel_id: String,
    pub contract_id: String,
    pub protocol_state: DlcProtocolState,
    pub trader_pubkey: String,
    pub timestamp: OffsetDateTime,
    pub protocol_type: DlcProtocolType,
}

pub(crate) fn get_dlc_protocol(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
) -> QueryResult<dlc_protocol::DlcProtocol> {
    let dlc_protocol: DlcProtocol = dlc_protocols::table
        .filter(dlc_protocols::protocol_id.eq(protocol_id.to_uuid()))
        .first(conn)?;

    let protocol_type = match dlc_protocol.protocol_type {
        DlcProtocolType::OpenChannel => {
            let trade_params = db::trade_params::get(conn, protocol_id)?;
            dlc_protocol::DlcProtocolType::OpenChannel { trade_params }
        }
        DlcProtocolType::OpenPosition => {
            let trade_params = db::trade_params::get(conn, protocol_id)?;
            dlc_protocol::DlcProtocolType::OpenPosition { trade_params }
        }
        DlcProtocolType::Settle => {
            let trade_params = db::trade_params::get(conn, protocol_id)?;
            dlc_protocol::DlcProtocolType::Settle { trade_params }
        }
        DlcProtocolType::Close => dlc_protocol::DlcProtocolType::Close {
            trader: PublicKey::from_str(&dlc_protocol.trader_pubkey).expect("valid public key"),
        },
        DlcProtocolType::ForceClose => dlc_protocol::DlcProtocolType::ForceClose {
            trader: PublicKey::from_str(&dlc_protocol.trader_pubkey).expect("valid public key"),
        },
        DlcProtocolType::Rollover => dlc_protocol::DlcProtocolType::Rollover {
            trader: PublicKey::from_str(&dlc_protocol.trader_pubkey).expect("valid public key"),
        },
        DlcProtocolType::ResizePosition => {
            let trade_params = db::trade_params::get(conn, protocol_id)?;
            dlc_protocol::DlcProtocolType::ResizePosition { trade_params }
        }
    };

    let protocol = dlc_protocol::DlcProtocol {
        id: dlc_protocol.protocol_id.into(),
        timestamp: dlc_protocol.timestamp,
        channel_id: DlcChannelId::from_hex(&dlc_protocol.channel_id).expect("valid dlc channel id"),
        contract_id: ContractId::from_hex(&dlc_protocol.contract_id).expect("valid contract id"),
        trader: PublicKey::from_str(&dlc_protocol.trader_pubkey).expect("valid public key"),
        protocol_state: dlc_protocol.protocol_state.into(),
        protocol_type,
    };

    Ok(protocol)
}

pub(crate) fn set_dlc_protocol_state_to_failed(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
) -> QueryResult<()> {
    let affected_rows = diesel::update(dlc_protocols::table)
        .filter(dlc_protocols::protocol_id.eq(protocol_id.to_uuid()))
        .set((dlc_protocols::protocol_state.eq(DlcProtocolState::Failed),))
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}

pub(crate) fn set_dlc_protocol_state_to_success(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
    contract_id: &ContractId,
    channel_id: &DlcChannelId,
) -> QueryResult<()> {
    let affected_rows = diesel::update(dlc_protocols::table)
        .filter(dlc_protocols::protocol_id.eq(protocol_id.to_uuid()))
        .set((
            dlc_protocols::protocol_state.eq(DlcProtocolState::Success),
            dlc_protocols::contract_id.eq(hex::encode(contract_id)),
            dlc_protocols::channel_id.eq(hex::encode(channel_id)),
        ))
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}

pub(crate) fn create(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
    previous_protocol_id: Option<ProtocolId>,
    contract_id: &ContractId,
    channel_id: &DlcChannelId,
    protocol_type: dlc_protocol::DlcProtocolType,
    trader: &PublicKey,
) -> QueryResult<()> {
    let affected_rows = diesel::insert_into(dlc_protocols::table)
        .values(&(
            dlc_protocols::protocol_id.eq(protocol_id.to_uuid()),
            dlc_protocols::previous_protocol_id.eq(previous_protocol_id.map(|ppid| ppid.to_uuid())),
            dlc_protocols::contract_id.eq(hex::encode(contract_id)),
            dlc_protocols::channel_id.eq(hex::encode(channel_id)),
            dlc_protocols::protocol_state.eq(DlcProtocolState::Pending),
            dlc_protocols::trader_pubkey.eq(trader.to_string()),
            dlc_protocols::timestamp.eq(OffsetDateTime::now_utc()),
            dlc_protocols::protocol_type.eq(DlcProtocolType::from(protocol_type)),
        ))
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}

impl From<dlc_protocol::DlcProtocolState> for DlcProtocolState {
    fn from(value: dlc_protocol::DlcProtocolState) -> Self {
        match value {
            dlc_protocol::DlcProtocolState::Pending => DlcProtocolState::Pending,
            dlc_protocol::DlcProtocolState::Success => DlcProtocolState::Success,
            dlc_protocol::DlcProtocolState::Failed => DlcProtocolState::Failed,
        }
    }
}

impl From<DlcProtocolState> for dlc_protocol::DlcProtocolState {
    fn from(value: DlcProtocolState) -> Self {
        match value {
            DlcProtocolState::Pending => dlc_protocol::DlcProtocolState::Pending,
            DlcProtocolState::Success => dlc_protocol::DlcProtocolState::Success,
            DlcProtocolState::Failed => dlc_protocol::DlcProtocolState::Failed,
        }
    }
}

impl From<dlc_protocol::DlcProtocolType> for DlcProtocolType {
    fn from(value: dlc_protocol::DlcProtocolType) -> Self {
        match value {
            dlc_protocol::DlcProtocolType::OpenChannel { .. } => DlcProtocolType::OpenChannel,
            dlc_protocol::DlcProtocolType::OpenPosition { .. } => DlcProtocolType::OpenPosition,
            dlc_protocol::DlcProtocolType::Settle { .. } => DlcProtocolType::Settle,
            dlc_protocol::DlcProtocolType::Close { .. } => DlcProtocolType::Close,
            dlc_protocol::DlcProtocolType::ForceClose { .. } => DlcProtocolType::ForceClose,
            dlc_protocol::DlcProtocolType::Rollover { .. } => DlcProtocolType::Rollover,
            dlc_protocol::DlcProtocolType::ResizePosition { .. } => DlcProtocolType::ResizePosition,
        }
    }
}
