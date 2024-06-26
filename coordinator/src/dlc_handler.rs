use crate::db;
use crate::node::storage::NodeStorage;
use crate::storage::CoordinatorTenTenOneStorage;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use futures::future::RemoteHandle;
use futures::FutureExt;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use xxi_node::bitcoin_conversion::to_secp_pk_29;
use xxi_node::dlc_message::DlcMessage;
use xxi_node::dlc_message::SerializedDlcMessage;
use xxi_node::message_handler::TenTenOneMessage;
use xxi_node::node::dlc_channel::send_dlc_message;
use xxi_node::node::event::NodeEvent;
use xxi_node::node::Node;

/// The DlcHandler is responsible for sending dlc messages and marking received ones as
/// processed. It's main purpose is to ensure the following.
///
/// 1. Mark all received inbound messages as processed.
/// 2. Save the last outbound dlc message, so it can be resend on the next reconnect.
/// 3. Check if a receive message has already been processed and if so inform to skip the message.

#[derive(Clone)]
pub struct DlcHandler {
    node: Arc<
        Node<
            bdk_file_store::Store<bdk::wallet::ChangeSet>,
            CoordinatorTenTenOneStorage,
            NodeStorage,
        >,
    >,
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl DlcHandler {
    pub fn new(
        pool: Pool<ConnectionManager<PgConnection>>,
        node: Arc<
            Node<
                bdk_file_store::Store<bdk::wallet::ChangeSet>,
                CoordinatorTenTenOneStorage,
                NodeStorage,
            >,
        >,
    ) -> Self {
        DlcHandler { node, pool }
    }
}

/// [`spawn_handling_outbound_dlc_messages`] handles sending outbound dlc messages as well as
/// keeping track of what dlc messages have already been processed and what was the last outbound
/// dlc message so it can be resend on reconnect.
pub fn spawn_handling_outbound_dlc_messages(
    dlc_handler: DlcHandler,
    mut receiver: broadcast::Receiver<NodeEvent>,
) -> RemoteHandle<()> {
    let (fut, remote_handle) = async move {
        loop {
            match receiver.recv().await {
                Ok(NodeEvent::Connected { peer }) => {
                    if let Err(e) = dlc_handler.on_connect(peer) {
                        tracing::error!(peer=%peer, "Failed to process on connect event. {e:#}");
                    }
                }
                Ok(NodeEvent::SendDlcMessage { peer, msg }) => {
                    if let Err(e) = dlc_handler.send_dlc_message(peer, msg) {
                        tracing::error!(peer=%peer, "Failed to process send dlc message event. {e:#}");
                    }
                }
                Ok(NodeEvent::StoreDlcMessage { peer, msg }) => {
                    if let Err(e) = dlc_handler.store_dlc_message(peer, msg) {
                        tracing::error!(peer=%peer, "Failed to store dlc message. {e:#}");
                    }
                }
                Ok(NodeEvent::SendLastDlcMessage { peer }) => {
                    if let Err(e) = dlc_handler.send_last_dlc_message(peer) {
                        tracing::error!(peer=%peer, "Failed to send last dlc message. {e:#}")
                    }
                }
                Ok(NodeEvent::DlcChannelEvent { .. }) => {} // ignored
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!("Skipped {skipped} messages");
                }
                Err(RecvError::Closed) => {
                    tracing::error!("Lost connection to sender!");
                    break;
                }
            }
        }
    }.remote_handle();

    tokio::spawn(fut);

    remote_handle
}

impl DlcHandler {
    pub fn send_dlc_message(&self, peer: PublicKey, msg: TenTenOneMessage) -> Result<()> {
        self.store_dlc_message(peer, msg.clone())?;

        send_dlc_message(
            &self.node.dlc_message_handler,
            &self.node.peer_manager,
            peer,
            msg,
        );

        Ok(())
    }

    pub fn store_dlc_message(&self, peer: PublicKey, msg: TenTenOneMessage) -> Result<()> {
        let mut conn = self.pool.get()?;

        let serialized_outbound_message = SerializedDlcMessage::try_from(&msg)?;
        let outbound_msg = DlcMessage::new(peer, serialized_outbound_message.clone(), false)?;

        db::dlc_messages::insert(&mut conn, outbound_msg)?;
        db::last_outbound_dlc_message::upsert(&mut conn, &peer, serialized_outbound_message)
    }

    pub fn send_last_dlc_message(&self, peer: PublicKey) -> Result<()> {
        let mut conn = self.pool.get()?;

        let last_serialized_message = db::last_outbound_dlc_message::get(&mut conn, &peer)?;

        if let Some(last_serialized_message) = last_serialized_message {
            let message = TenTenOneMessage::try_from(&last_serialized_message)?;
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

    pub fn on_connect(&self, peer: PublicKey) -> Result<()> {
        let signed_dlc_channels = self.node.list_signed_dlc_channels()?;

        if let Some(SignedChannel {
            channel_id,
            state:
                SignedChannelState::CollaborativeCloseOffered {
                    is_offer: false, ..
                },
            ..
        }) = signed_dlc_channels
            .iter()
            .find(|c| c.counter_party == to_secp_pk_29(peer))
        {
            tracing::info!("Accepting pending dlc channel close offer.");
            // Pending dlc channel close offer with the intend to close the dlc channel
            // on-chain

            // TODO(bonomat): we should verify that the proposed amount is acceptable
            self.node
                .accept_dlc_channel_collaborative_close(channel_id)?;

            return Ok(());
        }

        self.send_last_dlc_message(peer)?;

        Ok(())
    }
}
