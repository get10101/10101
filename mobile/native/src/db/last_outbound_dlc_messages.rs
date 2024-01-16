use crate::db::dlc_messages::MessageType;
use crate::schema;
use crate::schema::dlc_messages;
use crate::schema::last_outbound_dlc_messages;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::AsChangeset;
use diesel::ExpressionMethods;
use diesel::Insertable;
use diesel::JoinOnDsl;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use diesel::SqliteConnection;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use time::OffsetDateTime;

#[derive(Insertable, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = last_outbound_dlc_messages)]
pub(crate) struct LastOutboundDlcMessage {
    pub peer_id: String,
    pub message_hash: String,
    pub message: String,
    pub timestamp: i64,
}

impl LastOutboundDlcMessage {
    pub(crate) fn get(
        conn: &mut SqliteConnection,
        peer_id: &PublicKey,
    ) -> QueryResult<Option<SerializedDlcMessage>> {
        let last_outbound_dlc_message = last_outbound_dlc_messages::table
            .inner_join(
                dlc_messages::table
                    .on(dlc_messages::message_hash.eq(last_outbound_dlc_messages::message_hash)),
            )
            .filter(last_outbound_dlc_messages::peer_id.eq(peer_id.to_string()))
            .select((
                dlc_messages::message_type,
                last_outbound_dlc_messages::message,
            ))
            .first::<(MessageType, String)>(conn)
            .optional()?;

        let serialized_dlc_message =
            last_outbound_dlc_message.map(|(message_type, message)| SerializedDlcMessage {
                message,
                message_type: ln_dlc_node::dlc_message::DlcMessageType::from(message_type),
            });

        Ok(serialized_dlc_message)
    }

    pub(crate) fn upsert(
        conn: &mut SqliteConnection,
        peer_id: &PublicKey,
        sdm: SerializedDlcMessage,
    ) -> Result<()> {
        let values = (
            last_outbound_dlc_messages::peer_id.eq(peer_id.to_string()),
            last_outbound_dlc_messages::message_hash.eq(sdm.generate_hash().to_string()),
            last_outbound_dlc_messages::message.eq(sdm.message),
            last_outbound_dlc_messages::timestamp.eq(OffsetDateTime::now_utc().unix_timestamp()),
        );
        let affected_rows = diesel::insert_into(last_outbound_dlc_messages::table)
            .values(&values.clone())
            .on_conflict(schema::last_outbound_dlc_messages::peer_id)
            .do_update()
            .set(values)
            .execute(conn)?;

        ensure!(
            affected_rows > 0,
            "Could not upsert last outbound dlc messages"
        );

        Ok(())
    }
}
