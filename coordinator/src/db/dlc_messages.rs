use crate::schema;
use crate::schema::dlc_messages;
use crate::schema::sql_types::MessageSubTypeType;
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
    OnChain,
    Channel,
}

impl QueryId for MessageTypeType {
    type QueryId = MessageTypeType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = MessageSubTypeType)]
pub(crate) enum MessageSubType {
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

impl QueryId for MessageSubTypeType {
    type QueryId = MessageSubTypeType;
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
    pub message_sub_type: MessageSubType,
    pub timestamp: OffsetDateTime,
}

pub(crate) fn get(conn: &mut PgConnection, message_hash: u64) -> QueryResult<Option<DlcMessage>> {
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
            message_hash: value.message_hash.to_string(),
            peer_id: value.peer_id.to_string(),
            message_type: MessageType::from(value.clone().message_type),
            message_sub_type: MessageSubType::from(value.message_type),
            timestamp: value.timestamp,
            inbound: value.inbound,
        }
    }
}

impl From<ln_dlc_node::dlc_message::DlcMessageType> for MessageType {
    fn from(value: ln_dlc_node::dlc_message::DlcMessageType) -> Self {
        match value {
            ln_dlc_node::dlc_message::DlcMessageType::OnChain(_) => Self::OnChain,
            ln_dlc_node::dlc_message::DlcMessageType::Channel(_) => Self::Channel,
        }
    }
}

impl From<ln_dlc_node::dlc_message::DlcMessageType> for MessageSubType {
    fn from(value: ln_dlc_node::dlc_message::DlcMessageType) -> Self {
        let message_sub_type = match value {
            ln_dlc_node::dlc_message::DlcMessageType::OnChain(message_sub_type) => message_sub_type,
            ln_dlc_node::dlc_message::DlcMessageType::Channel(message_sub_type) => message_sub_type,
        };
        MessageSubType::from(message_sub_type)
    }
}

impl From<ln_dlc_node::dlc_message::DlcMessageSubType> for MessageSubType {
    fn from(value: ln_dlc_node::dlc_message::DlcMessageSubType) -> Self {
        match value {
            ln_dlc_node::dlc_message::DlcMessageSubType::Offer => Self::Offer,
            ln_dlc_node::dlc_message::DlcMessageSubType::Accept => Self::Accept,
            ln_dlc_node::dlc_message::DlcMessageSubType::Sign => Self::Sign,
            ln_dlc_node::dlc_message::DlcMessageSubType::SettleOffer => Self::SettleOffer,
            ln_dlc_node::dlc_message::DlcMessageSubType::SettleAccept => Self::SettleAccept,
            ln_dlc_node::dlc_message::DlcMessageSubType::SettleConfirm => Self::SettleConfirm,
            ln_dlc_node::dlc_message::DlcMessageSubType::SettleFinalize => Self::SettleFinalize,
            ln_dlc_node::dlc_message::DlcMessageSubType::RenewOffer => Self::RenewOffer,
            ln_dlc_node::dlc_message::DlcMessageSubType::RenewAccept => Self::RenewAccept,
            ln_dlc_node::dlc_message::DlcMessageSubType::RenewConfirm => Self::RenewConfirm,
            ln_dlc_node::dlc_message::DlcMessageSubType::RenewFinalize => Self::RenewFinalize,
            ln_dlc_node::dlc_message::DlcMessageSubType::RenewRevoke => Self::RenewRevoke,
            ln_dlc_node::dlc_message::DlcMessageSubType::CollaborativeCloseOffer => {
                Self::CollaborativeCloseOffer
            }
            ln_dlc_node::dlc_message::DlcMessageSubType::Reject => Self::Reject,
        }
    }
}

impl From<DlcMessage> for ln_dlc_node::dlc_message::DlcMessage {
    fn from(value: DlcMessage) -> Self {
        let dlc_message_sub_type =
            ln_dlc_node::dlc_message::DlcMessageSubType::from(value.clone().message_sub_type);
        let dlc_message_type = match &value.message_type {
            MessageType::OnChain => {
                ln_dlc_node::dlc_message::DlcMessageType::OnChain(dlc_message_sub_type)
            }
            MessageType::Channel => {
                ln_dlc_node::dlc_message::DlcMessageType::Channel(dlc_message_sub_type)
            }
        };

        Self {
            message_hash: u64::from_str(&value.message_hash).expect("valid u64"),
            inbound: value.inbound,
            message_type: dlc_message_type,
            peer_id: PublicKey::from_str(&value.peer_id).expect("valid public key"),
            timestamp: value.timestamp,
        }
    }
}

impl From<MessageSubType> for ln_dlc_node::dlc_message::DlcMessageSubType {
    fn from(value: MessageSubType) -> Self {
        match value {
            MessageSubType::Offer => ln_dlc_node::dlc_message::DlcMessageSubType::Offer,
            MessageSubType::Accept => ln_dlc_node::dlc_message::DlcMessageSubType::Accept,
            MessageSubType::Sign => ln_dlc_node::dlc_message::DlcMessageSubType::Sign,
            MessageSubType::SettleOffer => ln_dlc_node::dlc_message::DlcMessageSubType::SettleOffer,
            MessageSubType::SettleAccept => {
                ln_dlc_node::dlc_message::DlcMessageSubType::SettleAccept
            }
            MessageSubType::SettleConfirm => {
                ln_dlc_node::dlc_message::DlcMessageSubType::SettleConfirm
            }
            MessageSubType::SettleFinalize => {
                ln_dlc_node::dlc_message::DlcMessageSubType::SettleFinalize
            }
            MessageSubType::RenewOffer => ln_dlc_node::dlc_message::DlcMessageSubType::RenewOffer,
            MessageSubType::RenewAccept => ln_dlc_node::dlc_message::DlcMessageSubType::RenewAccept,
            MessageSubType::RenewConfirm => {
                ln_dlc_node::dlc_message::DlcMessageSubType::RenewConfirm
            }
            MessageSubType::RenewFinalize => {
                ln_dlc_node::dlc_message::DlcMessageSubType::RenewFinalize
            }
            MessageSubType::RenewRevoke => ln_dlc_node::dlc_message::DlcMessageSubType::RenewRevoke,
            MessageSubType::CollaborativeCloseOffer => {
                ln_dlc_node::dlc_message::DlcMessageSubType::CollaborativeCloseOffer
            }
            MessageSubType::Reject => ln_dlc_node::dlc_message::DlcMessageSubType::Reject,
        }
    }
}
