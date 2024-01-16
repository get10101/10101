use crate::db;
use crate::ln_dlc::node::NodeStorage;
use crate::storage::TenTenOneNodeStorage;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_messages::Message;
use ln_dlc_node::dlc_message::DlcMessage;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use ln_dlc_node::node::dlc_channel::send_dlc_message;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::rust_dlc_manager::channel::offered_channel::OfferedChannel;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannel;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannelState;
use ln_dlc_node::node::rust_dlc_manager::channel::Channel;
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
        if let Some(channel) = self.node.list_dlc_channels()?.first() {
            match channel {
                Channel::Offered(OfferedChannel {
                    temporary_channel_id,
                    ..
                }) => {
                    tracing::info!("Accepting pending dlc channel offer.");
                    // Pending dlc channel offer not yet confirmed on-chain
                    if let Err(e) = self.node.accept_dlc_channel_offer(temporary_channel_id) {
                        tracing::error!("Failed to accept pending dlc channel offer. {e:#}");
                        tracing::warn!("Rejecting pending dlc channel offer!");
                        self.node.reject_dlc_channel_offer(temporary_channel_id)?;
                    }

                    return Ok(());
                }
                Channel::Signed(SignedChannel {
                    channel_id,
                    state: SignedChannelState::SettledReceived { .. },
                    ..
                }) => {
                    tracing::info!("Accepting pending dlc channel settle offer.");
                    // Pending dlc channel settle offer with a dlc channel already confirmed
                    // on-chain
                    self.node
                        .accept_dlc_channel_collaborative_settlement(channel_id)?;

                    return Ok(());
                }
                Channel::Signed(SignedChannel {
                    channel_id: _,
                    state: SignedChannelState::RenewOffered { .. },
                    ..
                }) => {
                    // Pending dlc channel renew (resize) offer with a dlc channel already confirmed
                    // on-chain

                    // TODO: implement with resizing a position.

                    return Ok(());
                }
                Channel::Signed(SignedChannel {
                    channel_id,
                    state: SignedChannelState::CollaborativeCloseOffered { .. },
                    ..
                }) => {
                    tracing::info!("Accepting pending dlc channel close offer.");
                    // Pending dlc channel close offer with the intend to close the dlc channel
                    // on-chain

                    // TODO(bonomat): we should verify that the proposed amount is acceptable
                    self.node
                        .accept_dlc_channel_collaborative_close(channel_id)?;

                    return Ok(());
                }
                _ => {}
            }
        }

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
}
