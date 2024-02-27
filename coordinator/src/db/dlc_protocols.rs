use crate::dlc_protocol;
use crate::dlc_protocol::ProtocolId;
use crate::schema::dlc_protocols;
use crate::schema::sql_types::ProtocolStateType;
use bitcoin::hashes::hex::FromHex;
use bitcoin::hashes::hex::ToHex;
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
}

pub(crate) fn get_dlc_protocol(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
) -> QueryResult<dlc_protocol::DlcProtocol> {
    let contract_transaction: DlcProtocol = dlc_protocols::table
        .filter(dlc_protocols::protocol_id.eq(protocol_id.to_uuid()))
        .first(conn)?;

    Ok(dlc_protocol::DlcProtocol::from(contract_transaction))
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
    contract_id: ContractId,
    channel_id: DlcChannelId,
) -> QueryResult<()> {
    let affected_rows = diesel::update(dlc_protocols::table)
        .filter(dlc_protocols::protocol_id.eq(protocol_id.to_uuid()))
        .set((
            dlc_protocols::protocol_state.eq(DlcProtocolState::Success),
            dlc_protocols::contract_id.eq(contract_id.to_hex()),
            dlc_protocols::channel_id.eq(channel_id.to_hex()),
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
    contract_id: ContractId,
    channel_id: DlcChannelId,
    trader: &PublicKey,
) -> QueryResult<()> {
    let affected_rows = diesel::insert_into(dlc_protocols::table)
        .values(&(
            dlc_protocols::protocol_id.eq(protocol_id.to_uuid()),
            dlc_protocols::previous_protocol_id.eq(previous_protocol_id.map(|ppid| ppid.to_uuid())),
            dlc_protocols::contract_id.eq(contract_id.to_hex()),
            dlc_protocols::channel_id.eq(channel_id.to_hex()),
            dlc_protocols::protocol_state.eq(DlcProtocolState::Pending),
            dlc_protocols::trader_pubkey.eq(trader.to_string()),
            dlc_protocols::timestamp.eq(OffsetDateTime::now_utc()),
        ))
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}

impl From<DlcProtocol> for dlc_protocol::DlcProtocol {
    fn from(value: DlcProtocol) -> Self {
        dlc_protocol::DlcProtocol {
            id: value.protocol_id.into(),
            timestamp: value.timestamp,
            channel_id: DlcChannelId::from_hex(&value.channel_id).expect("valid dlc channel id"),
            contract_id: ContractId::from_hex(&value.contract_id).expect("valid contract id"),
            trader: PublicKey::from_str(&value.trader_pubkey).expect("valid public key"),
            protocol_state: value.protocol_state.into(),
        }
    }
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
