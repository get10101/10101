use crate::db;
use crate::ln_dlc::node::NodeStorage;
use crate::storage::TenTenOneNodeStorage;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::SqliteConnection;
use dlc_messages::Message;
use ln_dlc_node::dlc_message::DlcMessage;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use ln_dlc_node::node::dlc_channel::send_dlc_message;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::Node;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// The DlcHandler is responsible for sending dlc messages and marking received ones as
/// processed. It's main purpose is to ensure the following.
///
/// 1. Mark all received inbound messages as processed.
/// 2. Save the last outbound dlc message, so it can be resend on the next reconnect.
/// 3. Check if a receive message has already been processed and if so inform to skip the message.

#[derive(Clone)]
pub struct DlcHandler {
    node: Arc<Node<TenTenOneNodeStorage, NodeStorage>>,
}

impl DlcHandler {
    pub fn new(node: Arc<Node<TenTenOneNodeStorage, NodeStorage>>) -> Self {
        DlcHandler { node }
    }
}
pub async fn handle_dlc_messages(
    dlc_handler: DlcHandler,
    mut receiver: broadcast::Receiver<NodeEvent>,
) {
    loop {
        match receiver.recv().await {
            Ok(NodeEvent::Connected { peer }) => {
                if let Err(e) = dlc_handler.on_connect(peer) {
                    tracing::error!(peer=%peer, "Failed to process on connect event. {e:#}");
                }
            }
            Ok(NodeEvent::SendDlcMessage { peer, msg }) => {
                if let Err(e) = dlc_handler.send_dlc_message(peer, msg) {
                    tracing::error!(peer=%peer, "Failed to process end dlc message event. {e:#}");
                }
            }
            Err(RecvError::Lagged(skipped)) => {
                tracing::warn!("Skipped {skipped} messages");
            }
            Err(RecvError::Closed) => {
                tracing::error!("Lost connection to sender!");
                break;
            }
        }
    }
}

impl DlcHandler {
    pub fn send_dlc_message(&self, peer: PublicKey, msg: Message) -> Result<()> {
        let mut conn = db::connection()?;

        let serialized_outbound_message = SerializedDlcMessage::try_from(&msg)?;
        let outbound_msg = DlcMessage::new(peer, serialized_outbound_message.clone(), false)?;

        db::dlc_messages::DlcMessage::insert(&mut conn, outbound_msg)?;
        db::last_outbound_dlc_messages::LastOutboundDlcMessage::upsert(
            &mut conn,
            &peer,
            serialized_outbound_message,
        )?;

        send_dlc_message(
            &self.node.dlc_message_handler,
            &self.node.peer_manager,
            peer,
            msg,
        );

        Ok(())
    }

    pub fn on_connect(&self, peer: PublicKey) -> Result<()> {
        let mut conn = db::connection()?;
        let last_outbound_serialized_dlc_message =
            db::last_outbound_dlc_messages::LastOutboundDlcMessage::get(&mut conn, &peer)?;

        if let Some(last_outbound_serialized_dlc_message) = last_outbound_serialized_dlc_message {
            tracing::debug!(%peer, ?last_outbound_serialized_dlc_message.message_type, "Sending last dlc message");

            let message = Message::try_from(&last_outbound_serialized_dlc_message)?;
            send_dlc_message(
                &self.node.dlc_message_handler,
                &self.node.peer_manager,
                peer,
                message,
            );
        } else {
            tracing::debug!(%peer, "No last dlc message found. Nothing todo.");
        }

        Ok(())
    }
    // Returns either the dlc message step or return none, if the dlc message has already been
    // processed.
    pub fn start_dlc_message_step(
        conn: &mut SqliteConnection,
        msg: &Message,
        peer_id: PublicKey,
    ) -> Result<Option<DlcMessageStep>> {
        let serialized_inbound_message = SerializedDlcMessage::try_from(msg)?;
        let inbound_msg = DlcMessage::new(peer_id, serialized_inbound_message, true)?;

        let dlc_message_step =
            match db::dlc_messages::DlcMessage::get(conn, inbound_msg.message_hash)? {
                Some(_) => None, // the dlc message has already been processed, no step necessary.
                None => Some(DlcMessageStep {
                    inbound_msg,
                    peer_id,
                }),
            };

        Ok(dlc_message_step)
    }
}

pub struct DlcMessageStep {
    pub peer_id: PublicKey,
    pub inbound_msg: DlcMessage,
}

impl DlcMessageStep {
    /// Finishes the current dlc step by storing the received inbound message as processed and
    /// caching the last outbound dlc message (if any) into the database.
    pub fn finish(&self, conn: &mut SqliteConnection, response: &Option<Message>) -> Result<()> {
        tracing::debug!("Marking the received message as processed");

        db::dlc_messages::DlcMessage::insert(conn, self.inbound_msg.clone())?;

        if let Some(resp) = response {
            tracing::debug!("Persisting last outbound dlc message");
            let serialized_outbound_message = SerializedDlcMessage::try_from(resp)?;
            let outbound_msg =
                DlcMessage::new(self.peer_id, serialized_outbound_message.clone(), false)?;

            db::dlc_messages::DlcMessage::insert(conn, outbound_msg)?;
            db::last_outbound_dlc_messages::LastOutboundDlcMessage::upsert(
                conn,
                &self.peer_id,
                serialized_outbound_message,
            )?;
        }

        Ok(())
    }
}
