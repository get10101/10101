use crate::schema;
use crate::schema::dlc_messages;
use crate::schema::sql_types::MessageTypeType;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::query_builder::QueryId;
use diesel::AsChangeset;
use diesel::AsExpression;
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
use std::any::TypeId;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = MessageTypeType)]
pub(crate) enum MessageType {
    Offer,
    Accept,
    Sign,
    SettleOffer,
    SettleAccept,
    SettleConfirm,
    SettleFinalize,
    RenewOffer,
    RenewAccept,
    RenewConfirm,
    RenewFinalize,
    RenewRevoke,
    CollaborativeCloseOffer,
    Reject,
}

impl QueryId for MessageTypeType {
    type QueryId = MessageTypeType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

#[derive(Insertable, QueryableByName, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = dlc_messages)]
pub(crate) struct DlcMessage {
    pub message_hash: String,
    pub inbound: bool,
    pub peer_id: String,
    pub message_type: MessageType,
    pub timestamp: OffsetDateTime,
}

pub(crate) fn get(conn: &mut PgConnection, message_hash: &str) -> QueryResult<Option<DlcMessage>> {
    dlc_messages::table
        .filter(dlc_messages::message_hash.eq(message_hash.to_string()))
        .first::<DlcMessage>(conn)
        .optional()
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    dlc_message: ln_dlc_node::dlc_message::DlcMessage,
) -> Result<()> {
    let affected_rows = diesel::insert_into(schema::dlc_messages::table)
        .values(DlcMessage::from(dlc_message))
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert dlc message");

    Ok(())
}

impl From<ln_dlc_node::dlc_message::DlcMessage> for DlcMessage {
    fn from(value: ln_dlc_node::dlc_message::DlcMessage) -> Self {
        Self {
            message_hash: value.message_hash,
            peer_id: value.peer_id.to_string(),
            message_type: MessageType::from(value.message_type),
            timestamp: value.timestamp,
            inbound: value.inbound,
        }
    }
}

impl From<ln_dlc_node::dlc_message::DlcMessageType> for MessageType {
    fn from(value: ln_dlc_node::dlc_message::DlcMessageType) -> Self {
        match value {
            ln_dlc_node::dlc_message::DlcMessageType::Offer => Self::Offer,
            ln_dlc_node::dlc_message::DlcMessageType::Accept => Self::Accept,
            ln_dlc_node::dlc_message::DlcMessageType::Sign => Self::Sign,
            ln_dlc_node::dlc_message::DlcMessageType::SettleOffer => Self::SettleOffer,
            ln_dlc_node::dlc_message::DlcMessageType::SettleAccept => Self::SettleAccept,
            ln_dlc_node::dlc_message::DlcMessageType::SettleConfirm => Self::SettleConfirm,
            ln_dlc_node::dlc_message::DlcMessageType::SettleFinalize => Self::SettleFinalize,
            ln_dlc_node::dlc_message::DlcMessageType::RenewOffer => Self::RenewOffer,
            ln_dlc_node::dlc_message::DlcMessageType::RenewAccept => Self::RenewAccept,
            ln_dlc_node::dlc_message::DlcMessageType::RenewConfirm => Self::RenewConfirm,
            ln_dlc_node::dlc_message::DlcMessageType::RenewFinalize => Self::RenewFinalize,
            ln_dlc_node::dlc_message::DlcMessageType::RenewRevoke => Self::RenewRevoke,
            ln_dlc_node::dlc_message::DlcMessageType::CollaborativeCloseOffer => {
                Self::CollaborativeCloseOffer
            }
            ln_dlc_node::dlc_message::DlcMessageType::Reject => Self::Reject,
        }
    }
}

impl From<DlcMessage> for ln_dlc_node::dlc_message::DlcMessage {
    fn from(value: DlcMessage) -> Self {
        Self {
            message_hash: value.message_hash,
            inbound: value.inbound,
            message_type: ln_dlc_node::dlc_message::DlcMessageType::from(value.message_type),
            peer_id: PublicKey::from_str(&value.peer_id).expect("valid public key"),
            timestamp: value.timestamp,
        }
    }
}

impl From<MessageType> for ln_dlc_node::dlc_message::DlcMessageType {
    fn from(value: MessageType) -> Self {
        match value {
            MessageType::Offer => ln_dlc_node::dlc_message::DlcMessageType::Offer,
            MessageType::Accept => ln_dlc_node::dlc_message::DlcMessageType::Accept,
            MessageType::Sign => ln_dlc_node::dlc_message::DlcMessageType::Sign,
            MessageType::SettleOffer => ln_dlc_node::dlc_message::DlcMessageType::SettleOffer,
            MessageType::SettleAccept => ln_dlc_node::dlc_message::DlcMessageType::SettleAccept,
            MessageType::SettleConfirm => ln_dlc_node::dlc_message::DlcMessageType::SettleConfirm,
            MessageType::SettleFinalize => ln_dlc_node::dlc_message::DlcMessageType::SettleFinalize,
            MessageType::RenewOffer => ln_dlc_node::dlc_message::DlcMessageType::RenewOffer,
            MessageType::RenewAccept => ln_dlc_node::dlc_message::DlcMessageType::RenewAccept,
            MessageType::RenewConfirm => ln_dlc_node::dlc_message::DlcMessageType::RenewConfirm,
            MessageType::RenewFinalize => ln_dlc_node::dlc_message::DlcMessageType::RenewFinalize,
            MessageType::RenewRevoke => ln_dlc_node::dlc_message::DlcMessageType::RenewRevoke,
            MessageType::CollaborativeCloseOffer => {
                ln_dlc_node::dlc_message::DlcMessageType::CollaborativeCloseOffer
            }
            MessageType::Reject => ln_dlc_node::dlc_message::DlcMessageType::Reject,
        }
    }
}
