use crate::db;
use crate::dlc::node::Node;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use xxi_node::dlc_message::DlcMessage;
use xxi_node::dlc_message::SerializedDlcMessage;
use xxi_node::message_handler::TenTenOneMessage;
use xxi_node::node::dlc_channel::send_dlc_message;
use xxi_node::node::event::NodeEvent;
use xxi_node::node::rust_dlc_manager::channel::signed_channel::SignedChannel;
use xxi_node::node::rust_dlc_manager::channel::signed_channel::SignedChannelState;
use xxi_node::node::rust_dlc_manager::channel::Channel;

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
                    tracing::error!(peer=%peer, "Failed to send dlc message. {e:#}")
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
}

impl DlcHandler {
    pub fn send_dlc_message(&self, peer: PublicKey, msg: TenTenOneMessage) -> Result<()> {
        self.store_dlc_message(peer, msg.clone())?;

        send_dlc_message(
            &self.node.inner.dlc_message_handler,
            &self.node.inner.peer_manager,
            peer,
            msg,
        );

        Ok(())
    }

    pub fn store_dlc_message(&self, peer: PublicKey, msg: TenTenOneMessage) -> Result<()> {
        let mut conn = db::connection()?;

        let serialized_outbound_message = SerializedDlcMessage::try_from(&msg)?;
        let outbound_msg = DlcMessage::new(peer, serialized_outbound_message.clone(), false)?;

        db::dlc_messages::DlcMessage::insert(&mut conn, outbound_msg)?;
        db::last_outbound_dlc_messages::LastOutboundDlcMessage::upsert(
            &mut conn,
            &peer,
            serialized_outbound_message,
        )
    }

    pub fn send_last_dlc_message(&self, peer: PublicKey) -> Result<()> {
        let mut conn = db::connection()?;
        let last_serialized_message =
            db::last_outbound_dlc_messages::LastOutboundDlcMessage::get(&mut conn, &peer)?;

        if let Some(last_serialized_message) = last_serialized_message {
            let message = TenTenOneMessage::try_from(&last_serialized_message)?;
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
                channel_id = hex::encode(offered_channel.get_id()),
                "Rejecting pending dlc channel offer."
            );
            // Pending dlc channel offer not yet confirmed on-chain

            self.node
                .reject_dlc_channel_offer(None, &offered_channel.get_temporary_id())
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
                        .reject_settle_offer(None, channel_id)
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

                    // FIXME(holzeis): We need to be able to differentiate between a
                    // SignedChannelState::RenewOffered and a RolloverOffer. This differentiation
                    // currently only lives in 10101 and in the dlc messages, but not on the channel
                    // state. Hence at the moment we also reject pending `RolloverOffers` the same
                    // way we reject `RenewOffers`
                    self.node
                        .reject_renew_offer(None, channel_id)
                        .context("Failed to reject pending renew offer")?;

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::RecoverDlc(TaskStatus::Success),
                    ));

                    return Ok(());
                }
                SignedChannel {
                    channel_id,
                    state:
                        SignedChannelState::CollaborativeCloseOffered {
                            is_offer: false, ..
                        },
                    ..
                } => {
                    tracing::info!("Accepting pending dlc channel close offer.");
                    // Pending dlc channel close offer with the intend to close the dlc channel
                    // on-chain

                    // TODO(bonomat): we should verify that the proposed amount is acceptable
                    self.node
                        .inner
                        .accept_dlc_channel_collaborative_close(channel_id)?;

                    return Ok(());
                }
                signed_channel => {
                    // If the signed channel state is anything else but `Established`, `Settled` or
                    // `Closing` at reconnect. It means the protocol got interrupted.
                    if !matches!(
                        signed_channel.state,
                        SignedChannelState::Established { .. }
                            | SignedChannelState::Settled { .. }
                            | SignedChannelState::SettledClosing { .. }
                            | SignedChannelState::Closing { .. }
                            | SignedChannelState::CollaborativeCloseOffered { .. }
                    ) {
                        event::publish(&EventInternal::BackgroundNotification(
                            BackgroundTask::RecoverDlc(TaskStatus::Pending),
                        ));
                    }
                }
            };
        }

        self.send_last_dlc_message(peer)?;

        Ok(())
    }
}
