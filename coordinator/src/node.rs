use crate::db;
use crate::dlc_protocol;
use crate::dlc_protocol::ProtocolId;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::position::models::PositionState;
use crate::storage::CoordinatorTenTenOneStorage;
use crate::trade::websocket::InternalPositionUpdateMessage;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_messages::channel::AcceptChannel;
use dlc_messages::channel::Reject;
use dlc_messages::channel::RenewFinalize;
use dlc_messages::channel::SettleFinalize;
use dlc_messages::channel::SignChannel;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use xxi_node::bitcoin_conversion::to_secp_pk_29;
use xxi_node::bitcoin_conversion::to_secp_pk_30;
use xxi_node::commons::Message::RolloverError;
use xxi_node::commons::Message::TradeError;
use xxi_node::commons::TradingError;
use xxi_node::dlc_message::DlcMessage;
use xxi_node::dlc_message::SerializedDlcMessage;
use xxi_node::message_handler::TenTenOneAcceptChannel;
use xxi_node::message_handler::TenTenOneCollaborativeCloseOffer;
use xxi_node::message_handler::TenTenOneMessage;
use xxi_node::message_handler::TenTenOneMessageType;
use xxi_node::message_handler::TenTenOneReject;
use xxi_node::message_handler::TenTenOneRenewFinalize;
use xxi_node::message_handler::TenTenOneRolloverFinalize;
use xxi_node::message_handler::TenTenOneSettleFinalize;
use xxi_node::message_handler::TenTenOneSignChannel;
use xxi_node::node;
use xxi_node::node::event::NodeEvent;
use xxi_node::node::tentenone_message_name;
use xxi_node::node::RunningNode;

pub mod channel;
pub mod expired_positions;
pub mod liquidated_positions;
pub mod rollover;
pub mod storage;
pub mod unrealized_pnl;

#[derive(Debug, Clone)]
pub struct NodeSettings {
    // At times, we want to disallow opening new positions (e.g. before
    // scheduled upgrade)
    pub allow_opening_positions: bool,
    pub maintenance_margin_rate: f32,
    pub order_matching_fee_rate: f32,
}

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<
        node::Node<
            bdk_file_store::Store<bdk::wallet::ChangeSet>,
            CoordinatorTenTenOneStorage,
            NodeStorage,
        >,
    >,
    _running: Arc<RunningNode>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub settings: Arc<RwLock<NodeSettings>>,
    tx_position_feed: Sender<InternalPositionUpdateMessage>,
    trade_notifier: mpsc::Sender<OrderbookMessage>,
}

impl Node {
    pub fn new(
        inner: Arc<
            node::Node<
                bdk_file_store::Store<bdk::wallet::ChangeSet>,
                CoordinatorTenTenOneStorage,
                NodeStorage,
            >,
        >,
        running: RunningNode,
        pool: Pool<ConnectionManager<PgConnection>>,
        settings: NodeSettings,
        tx_position_feed: Sender<InternalPositionUpdateMessage>,
        trade_notifier: mpsc::Sender<OrderbookMessage>,
    ) -> Self {
        Self {
            inner,
            pool,
            settings: Arc::new(RwLock::new(settings)),
            _running: Arc::new(running),
            tx_position_feed,
            trade_notifier,
        }
    }

    /// Returns true or false, whether the given peer_id is connected with us.
    pub fn is_connected(&self, peer_id: PublicKey) -> bool {
        self.inner
            .peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|(id, _)| *id == to_secp_pk_29(peer_id))
    }

    pub fn process_incoming_dlc_messages(&self) {
        if !self
            .inner
            .dlc_message_handler
            .has_pending_messages_to_process()
        {
            return;
        }

        let messages = self
            .inner
            .dlc_message_handler
            .get_and_clear_received_messages();

        for (node_id, msg) in messages {
            let msg_name = tentenone_message_name(&msg);
            if let Err(e) = self.process_dlc_message(to_secp_pk_30(node_id), &msg) {
                if let Err(e) = self.set_dlc_protocol_to_failed(&msg) {
                    tracing::error!(
                        from = %node_id,
                        "Failed to set dlc protocol to failed. {e:#}"
                    );
                }

                tokio::spawn({
                    let trade_notifier = self.trade_notifier.clone();
                    let error = TradingError::Other(format!("{e:#}"));
                    async move {
                        let message = match msg.get_tentenone_message_type() {
                            TenTenOneMessageType::Trade
                            | TenTenOneMessageType::Expire
                            | TenTenOneMessageType::Liquidate => {
                                if let Some(order_id) = msg.get_order_id() {
                                    OrderbookMessage::TraderMessage {
                                        trader_id: to_secp_pk_30(node_id),
                                        message: TradeError { order_id, error },
                                        notification: None,
                                    }
                                } else {
                                    tracing::warn!("Could not send trade error to user due to missing order id");
                                    return;
                                }
                            }
                            TenTenOneMessageType::Rollover => OrderbookMessage::TraderMessage {
                                trader_id: to_secp_pk_30(node_id),
                                message: RolloverError { error },
                                notification: None,
                            },
                            TenTenOneMessageType::Other => {
                                tracing::debug!("Not sending errors to the app unrelated to a trade or rollover.");
                                return;
                            }
                        };

                        if let Err(e) = trade_notifier.send(message).await {
                            tracing::error!("Failed to send trade error to user. Error: {e:#}");
                        }
                    }
                });

                tracing::error!(
                    from = %node_id,
                    kind = %msg_name,
                    "Failed to process DLC message: {e:#}"
                );
            }
        }
    }

    fn set_dlc_protocol_to_failed(&self, msg: &TenTenOneMessage) -> Result<()> {
        if let Some(protocol_id) = msg.get_reference_id() {
            let protocol_id = ProtocolId::try_from(protocol_id)?;
            dlc_protocol::DlcProtocolExecutor::new(self.pool.clone())
                .fail_dlc_protocol(protocol_id)?;
        }

        Ok(())
    }

    /// Process an incoming [`TenTenOneMessage`] and update the 10101 position accordingly.
    ///
    /// - Any other kind of message will be ignored.
    /// - Any message that has already been processed will be skipped.
    ///
    /// Offers such as [`TenTenOneMessage::Offer`], [`TenTenOneMessage::SettleOffer`],
    /// [`TenTenOneMessage::RolloverOffer`], [`TenTenOneMessage::CollaborativeCloseOffer`] and
    /// [`TenTenOneMessage::RenewOffer`] are automatically accepted. Unless the maturity date of
    /// the offer is already outdated.
    ///
    /// FIXME(holzeis): This function manipulates different data objects from different data sources
    /// and should use a transaction to make all changes atomic. Not doing so risks ending up in an
    /// inconsistent state. One way of fixing that could be to: (1) use a single data source for the
    /// 10101 data and the `rust-dlc` data; (2) wrap the function into a DB transaction which can be
    /// atomically rolled back on error or committed on success.
    fn process_dlc_message(&self, node_id: PublicKey, msg: &TenTenOneMessage) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %tentenone_message_name(msg),
            "Processing message"
        );

        let protocol_id = match msg.get_reference_id() {
            Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
            None => None,
        };

        tracing::debug!(
            from = %node_id,
            ?protocol_id,
            "Received message"
        );

        self.verify_collab_close_offer(&node_id, msg)?;

        let inbound_msg = {
            let mut conn = self.pool.get()?;
            let serialized_inbound_message = SerializedDlcMessage::try_from(msg)?;
            let inbound_msg = DlcMessage::new(node_id, serialized_inbound_message, true)?;
            match db::dlc_messages::get(&mut conn, &inbound_msg.message_hash)? {
                Some(_) => {
                    tracing::debug!(%node_id, kind=%tentenone_message_name(msg), "Received message that has already been processed, skipping.");
                    return Ok(());
                }
                None => inbound_msg,
            }
        };

        let resp = self
            .inner
            .process_tentenone_message(msg.clone(), node_id)
            .with_context(|| {
                format!(
                    "Failed to handle {} dlc message from {node_id}",
                    tentenone_message_name(msg)
                )
            })?;

        if let Some(msg) = resp.clone() {
            // store dlc message immediately so we do not lose the response if something
            // goes wrong afterwards.
            self.inner
                .event_handler
                .publish(NodeEvent::StoreDlcMessage { peer: node_id, msg });
        }

        {
            let mut conn = self.pool.get()?;
            db::dlc_messages::insert(&mut conn, inbound_msg)?;
        }

        match msg {
            TenTenOneMessage::RenewFinalize(TenTenOneRenewFinalize {
                renew_finalize:
                    RenewFinalize {
                        channel_id,
                        reference_id,
                        ..
                    },
                ..
            })
            | TenTenOneMessage::RolloverFinalize(TenTenOneRolloverFinalize {
                renew_finalize:
                    RenewFinalize {
                        channel_id,
                        reference_id,
                        ..
                    },
            }) => {
                let channel_id_hex_string = hex::encode(channel_id);

                let reference_id = match reference_id {
                    Some(reference_id) => *reference_id,
                    // If the app did not yet update to the latest version, it will not
                    // send us the reference id in the message. In that case we will
                    // have to look up the reference id ourselves from the channel.
                    // TODO(holzeis): Remove this fallback handling once not needed
                    // anymore.
                    None => self
                        .inner
                        .get_dlc_channel_by_id(channel_id)?
                        .get_reference_id()
                        .context("missing reference id")?,
                };
                let protocol_id = ProtocolId::try_from(reference_id)?;

                tracing::info!(
                    channel_id = channel_id_hex_string,
                    node_id = node_id.to_string(),
                    %protocol_id,
                    "DLC channel renew protocol was finalized"
                );

                let channel = self.inner.get_dlc_channel_by_id(channel_id)?;

                let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                protocol_executor.finish_dlc_protocol(
                    protocol_id,
                    &node_id,
                    channel.get_contract_id(),
                    channel_id,
                    self.tx_position_feed.clone(),
                )?;
            }
            TenTenOneMessage::SettleFinalize(TenTenOneSettleFinalize {
                settle_finalize:
                    SettleFinalize {
                        channel_id,
                        reference_id,
                        ..
                    },
                ..
            }) => {
                let channel_id_hex_string = hex::encode(channel_id);

                let reference_id = match reference_id {
                    Some(reference_id) => *reference_id,
                    // If the app did not yet update to the latest version, it will not
                    // send us the reference id in the message. In that case we will
                    // have to look up the reference id ourselves from the channel.
                    // TODO(holzeis): Remove this fallback handling once not needed
                    // anymore.
                    None => self
                        .inner
                        .get_dlc_channel_by_id(channel_id)?
                        .get_reference_id()
                        .context("missing reference id")?,
                };
                let protocol_id = ProtocolId::try_from(reference_id)?;

                tracing::info!(
                    channel_id = channel_id_hex_string,
                    node_id = node_id.to_string(),
                    %protocol_id,
                    "DLC channel settle protocol was finalized"
                );

                let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                protocol_executor.finish_dlc_protocol(
                    protocol_id,
                    &node_id,
                    // the settled signed channel does not have a contract
                    None,
                    channel_id,
                    self.tx_position_feed.clone(),
                )?;
            }
            TenTenOneMessage::CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer {
                collaborative_close_offer: close_offer,
            }) => {
                tracing::info!(
                    channel_id = hex::encode(close_offer.channel_id),
                    node_id = node_id.to_string(),
                    "Received an offer to collaboratively close a channel"
                );

                self.inner
                    .accept_dlc_channel_collaborative_close(&close_offer.channel_id)?;
            }
            TenTenOneMessage::Accept(TenTenOneAcceptChannel {
                accept_channel:
                    AcceptChannel {
                        temporary_channel_id,
                        reference_id,
                        ..
                    },
                ..
            }) => {
                let channel_id = match resp {
                    Some(TenTenOneMessage::Sign(TenTenOneSignChannel {
                        sign_channel: SignChannel { channel_id, .. },
                        ..
                    })) => channel_id,
                    _ => *temporary_channel_id,
                };

                let reference_id = match reference_id {
                    Some(reference_id) => *reference_id,
                    // If the app did not yet update to the latest version, it will not
                    // send us the reference id in the message. In that case we will
                    // have to look up the reference id ourselves from the channel.
                    // TODO(holzeis): Remove this fallback handling once not needed
                    // anymore.
                    None => self
                        .inner
                        .get_dlc_channel_by_id(&channel_id)?
                        .get_reference_id()
                        .context("missing reference id")?,
                };
                let protocol_id = ProtocolId::try_from(reference_id)?;

                tracing::info!(
                    channel_id = hex::encode(channel_id),
                    node_id = node_id.to_string(),
                    %protocol_id,
                    "DLC channel open protocol was finalized"
                );

                let channel = self.inner.get_dlc_channel_by_id(&channel_id)?;

                let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                protocol_executor.finish_dlc_protocol(
                    protocol_id,
                    &node_id,
                    channel.get_contract_id(),
                    &channel_id,
                    self.tx_position_feed.clone(),
                )?;
            }
            TenTenOneMessage::Reject(TenTenOneReject {
                reject:
                    Reject {
                        channel_id,
                        reference_id,
                        ..
                    },
            }) => {
                let channel_id_hex_string = hex::encode(channel_id);

                let reference_id = match reference_id {
                    Some(reference_id) => *reference_id,
                    // If the app did not yet update to the latest version, it will not
                    // send us the reference id in the message. In that case we will
                    // have to look up the reference id ourselves from the channel.
                    // TODO(holzeis): Remove this fallback handling once not needed
                    // anymore.
                    None => self
                        .inner
                        .get_dlc_channel_by_id(channel_id)?
                        .get_reference_id()
                        .context("missing reference id")?,
                };
                let protocol_id = ProtocolId::try_from(reference_id)?;

                let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                protocol_executor.fail_dlc_protocol(protocol_id)?;

                let channel = self.inner.get_dlc_channel_by_id(channel_id)?;
                let mut connection = self.pool.get()?;

                match channel {
                    Channel::Cancelled(_) => {
                        tracing::info!(
                            channel_id = channel_id_hex_string,
                            node_id = node_id.to_string(),
                            "DLC Channel offer has been rejected. Setting position to failed."
                        );

                        db::positions::Position::update_position_state(
                            &mut connection,
                            node_id.to_string(),
                            vec![PositionState::Proposed],
                            PositionState::Failed,
                        )?;
                    }
                    Channel::Signed(SignedChannel {
                        state: SignedChannelState::Established { .. },
                        ..
                    }) => {
                        // TODO(holzeis): Reverting the position state back from `Closing`
                        // to `Open` only works as long as we do not support resizing. This
                        // logic needs to be adapted when we implement resize.

                        tracing::info!(
                                    channel_id = channel_id_hex_string,
                                    node_id = node_id.to_string(),
                                    "DLC Channel settle offer has been rejected. Setting position to back to open."
                                );

                        db::positions::Position::update_closing_position(
                            &mut connection,
                            node_id.to_string(),
                            PositionState::Open,
                        )?;
                    }
                    Channel::Signed(SignedChannel {
                        state: SignedChannelState::Settled { .. },
                        ..
                    }) => {
                        tracing::info!(
                                    channel_id = channel_id_hex_string,
                                    node_id = node_id.to_string(),
                                    "DLC Channel renew offer has been rejected. Setting position to failed."
                                );

                        db::positions::Position::update_position_state(
                            &mut connection,
                            node_id.to_string(),
                            vec![PositionState::Proposed],
                            PositionState::Failed,
                        )?;
                    }
                    _ => {}
                }
            }
            _ => {}
        };

        if let Some(msg) = resp {
            // Everything has been processed successfully, we can safely send the last dlc message,
            // that has been stored before.
            tracing::info!(
                to = %node_id,
                kind = %tentenone_message_name(&msg),
                "Sending message"
            );

            self.inner
                .event_handler
                .publish(NodeEvent::SendLastDlcMessage { peer: node_id });
        }

        Ok(())
    }

    /// TODO(holzeis): We need to intercept the collaborative close offer before
    /// processing it in `rust-dlc` as it would otherwise overwrite the `own_payout`
    /// amount, which would prevent us from verifying the proposed payout amount.
    ///
    /// If the expected own payout amount does not match the offered own payout amount,
    /// we will simply ignore the offer.
    fn verify_collab_close_offer(&self, node_id: &PublicKey, msg: &TenTenOneMessage) -> Result<()> {
        let close_offer = match msg {
            TenTenOneMessage::CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer {
                collaborative_close_offer: close_offer,
            }) => close_offer,
            _ => return Ok(()),
        };

        let channel = self.inner.get_dlc_channel_by_id(&close_offer.channel_id)?;
        match channel {
            Channel::Signed(SignedChannel {
                state: SignedChannelState::Established { .. },
                channel_id,
                ..
            }) => {
                let channel_id_hex = hex::encode(channel_id);

                tracing::debug!(%node_id, channel_id = %channel_id_hex, "Ignoring dlc channel collaborative close offer");
                bail!("channel_id = {channel_id_hex}, node_id = {node_id}, state = Established Received DLC channel \
                        collaborative close offer in an unexpected signed channel state");
            }
            Channel::Signed(SignedChannel {
                state:
                    SignedChannelState::Settled {
                        own_payout: expected_own_payout,
                        ..
                    },
                channel_id,
                ..
            }) => {
                let offered_own_payout = close_offer.counter_payout;
                if expected_own_payout != offered_own_payout {
                    let channel_id_hex = hex::encode(channel_id);

                    // TODO(holzeis): Implement reject collaborative close offer flow https://github.com/get10101/10101/issues/2019
                    tracing::debug!(%node_id, channel_id = %channel_id_hex, "Ignoring dlc channel collaborative close offer");

                    bail!("node_id = {node_id}, channel_id = {channel_id_hex}, offered_own_payout = {offered_own_payout}, \
                            expected_own_payout = {expected_own_payout}, Received DLC channel collaborative close offer with an invalid payout");
                }
            }
            _ => {}
        };

        Ok(())
    }
}
