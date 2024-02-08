use crate::db;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::ln_dlc::node::Node;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use dlc_messages::Message;
use ln_dlc_node::dlc_message::DlcMessage;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use ln_dlc_node::node::dlc_channel::send_dlc_message;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannel;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannelState;
use ln_dlc_node::node::rust_dlc_manager::channel::Channel;
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
    node: Arc<Node>,
}

impl DlcHandler {
    pub fn new(node: Arc<Node>) -> Self {
        DlcHandler { node }
    }
}

/// Handles sending outbound dlc messages as well as keeping track of what
/// dlc messages have already been processed and what was the last outbound dlc message
/// so it can be resend on reconnect.
pub async fn handle_outbound_dlc_messages(
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
            &self.node.inner.dlc_message_handler,
            &self.node.inner.peer_manager,
            peer,
            msg,
        );

        Ok(())
    }

    /// Rejects all pending dlc channel offers. This is important as there might be several
    /// pending dlc channel offers due to a bug before we had fixed the reject handling properly,
    /// leaving the positions in proposes on the coordinator side.
    ///
    /// By rejecting them we ensure that all hanging dlc channel offers and positions are dealt
    /// with.
    pub fn reject_pending_dlc_channel_offers(&self) -> Result<()> {
        let dlc_channels = self.node.inner.list_dlc_channels()?;
        let offered_channels = dlc_channels
            .iter()
            .filter(|c| matches!(c, Channel::Offered(_)))
            .collect::<Vec<&Channel>>();

        if offered_channels.is_empty() {
            return Ok(());
        }

        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::RecoverDlc(TaskStatus::Pending),
        ));

        for offered_channel in offered_channels.iter() {
            tracing::info!(
                channel_id = offered_channel.get_id().to_hex(),
                "Rejecting pending dlc channel offer."
            );
            // Pending dlc channel offer not yet confirmed on-chain

            self.node
                .reject_dlc_channel_offer(&offered_channel.get_temporary_id())
                .context("Failed to reject pending dlc channel offer")?;
        }

        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::RecoverDlc(TaskStatus::Success),
        ));

        Ok(())
    }

    pub fn on_connect(&self, peer: PublicKey) -> Result<()> {
        self.reject_pending_dlc_channel_offers()?;

        if let Some(channel) = self.node.inner.list_signed_dlc_channels()?.first() {
            match channel {
                SignedChannel {
                    channel_id,
                    state: SignedChannelState::SettledReceived { .. },
                    ..
                } => {
                    tracing::info!("Rejecting pending dlc channel settle offer.");
                    // Pending dlc channel settle offer with a dlc channel already confirmed
                    // on-chain

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::RecoverDlc(TaskStatus::Pending),
                    ));

                    self.node
                        .reject_settle_offer(channel_id)
                        .context("Failed to reject pending settle offer")?;

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::RecoverDlc(TaskStatus::Success),
                    ));

                    return Ok(());
                }
                SignedChannel {
                    channel_id,
                    state: SignedChannelState::RenewOffered { .. },
                    ..
                } => {
                    tracing::info!("Rejecting pending dlc channel renew offer.");
                    // Pending dlc channel renew (resize) offer with a dlc channel already confirmed
                    // on-chain

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::RecoverDlc(TaskStatus::Pending),
                    ));

                    self.node
                        .reject_renew_offer(channel_id)
                        .context("Failed to reject pending renew offer")?;

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::RecoverDlc(TaskStatus::Success),
                    ));

                    return Ok(());
                }
                signed_channel => {
                    // If the signed channel state is anything else but `Established`, `Settled` or
                    // `Closing` at reconnect. It means the protocol got interrupted.
                    if !matches!(
                        signed_channel.state,
                        SignedChannelState::Established { .. }
                            | SignedChannelState::Settled { .. }
                            | SignedChannelState::Closing { .. }
                    ) {
                        event::publish(&EventInternal::BackgroundNotification(
                            BackgroundTask::RecoverDlc(TaskStatus::Pending),
                        ));
                    }
                }
            };
        }

        let mut conn = db::connection()?;
        let last_outbound_serialized_dlc_message =
            db::last_outbound_dlc_messages::LastOutboundDlcMessage::get(&mut conn, &peer)?;

        if let Some(last_outbound_serialized_dlc_message) = last_outbound_serialized_dlc_message {
            tracing::debug!(%peer, ?last_outbound_serialized_dlc_message.message_type, "Sending last dlc message");

            let message = Message::try_from(&last_outbound_serialized_dlc_message)?;
            send_dlc_message(
                &self.node.inner.dlc_message_handler,
                &self.node.inner.peer_manager,
                peer,
                message,
            );
        } else {
            tracing::debug!(%peer, "No last dlc message found. Nothing todo.");
        }

        Ok(())
    }
}
