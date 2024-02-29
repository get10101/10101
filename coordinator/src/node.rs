use crate::db;
use crate::dlc_protocol;
use crate::dlc_protocol::ProtocolId;
use crate::message::OrderbookMessage;
use crate::node::storage::NodeStorage;
use crate::position::models::PositionState;
use crate::storage::CoordinatorTenTenOneStorage;
use crate::trade::TradeExecutor;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::TradeAndChannelParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_messages::channel::AcceptChannel;
use dlc_messages::channel::RenewFinalize;
use dlc_messages::channel::SettleFinalize;
use dlc_messages::channel::SignChannel;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_29;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_30;
use ln_dlc_node::dlc_message::DlcMessage;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::RunningNode;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

pub mod expired_positions;
pub mod rollover;
pub mod storage;
pub mod unrealized_pnl;

#[derive(Debug, Clone)]
pub struct NodeSettings {
    // At times, we want to disallow opening new positions (e.g. before
    // scheduled upgrade)
    pub allow_opening_positions: bool,
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
    settings: Arc<RwLock<NodeSettings>>,
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
    ) -> Self {
        Self {
            inner,
            pool,
            settings: Arc::new(RwLock::new(settings)),
            _running: Arc::new(running),
        }
    }

    /// Returns true or false, whether we can find an usable channel with the provided trader.
    ///
    /// Note, we use the usable channel to implicitely check if the user is connected, as it
    /// wouldn't be usable otherwise.
    pub fn is_connected(&self, trader: &PublicKey) -> bool {
        let usable_channels = self.inner.channel_manager.list_usable_channels();
        let usable_channels = usable_channels
            .iter()
            .filter(|channel| {
                channel.is_usable && channel.counterparty.node_id == to_secp_pk_29(*trader)
            })
            .collect::<Vec<_>>();

        if usable_channels.len() > 1 {
            tracing::warn!(peer_id=%trader, "Found more than one usable channel with trader");
        }
        !usable_channels.is_empty()
    }

    pub async fn trade(
        &self,
        notifier: mpsc::Sender<OrderbookMessage>,
        params: TradeAndChannelParams,
    ) -> Result<()> {
        let trade_executor = TradeExecutor::new(
            self.inner.clone(),
            self.pool.clone(),
            self.settings.clone(),
            notifier,
        );

        tokio::spawn(async move {
            trade_executor.execute(&params).await;
        });

        Ok(())
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
            let msg_name = dlc_message_name(&msg);
            if let Err(e) = self.process_dlc_message(to_secp_pk_30(node_id), &msg) {
                if let Err(e) = self.set_dlc_protocol_to_failed(&msg) {
                    tracing::error!(
                        from = %node_id,
                        "Failed to set dlc protocol to failed. {e:#}"
                    );
                }

                tracing::error!(
                    from = %node_id,
                    kind = %msg_name,
                    "Failed to process DLC message: {e:#}"
                );
            }
        }
    }

    fn set_dlc_protocol_to_failed(&self, msg: &Message) -> Result<()> {
        let msg = match msg {
            Message::OnChain(_) => return Ok(()),
            Message::Channel(msg) => msg,
            Message::SubChannel(_) => return Ok(()),
        };

        if let Some(protocol_id) = msg.get_reference_id() {
            let protocol_id = ProtocolId::try_from(protocol_id)?;
            dlc_protocol::DlcProtocolExecutor::new(self.pool.clone())
                .fail_dlc_protocol(protocol_id)?;
        }

        Ok(())
    }

    /// Process an incoming [`Message::Channel`] and update the 10101 position accordingly.
    ///
    /// - Any other kind of message will be ignored.
    /// - Any message that has already been processed will be skipped.
    ///
    /// Offers such as [`ChannelMessage::Offer`], [`ChannelMessage::SettleOffer`],
    /// [`ChannelMessage::CollaborativeCloseOffer`] and [`ChannelMessage::RenewOffer`] are
    /// automatically accepted. Unless the maturity date of the offer is already outdated.
    ///
    /// FIXME(holzeis): This function manipulates different data objects from different data sources
    /// and should use a transaction to make all changes atomic. Not doing so risks ending up in an
    /// inconsistent state. One way of fixing that could be to: (1) use a single data source for the
    /// 10101 data and the `rust-dlc` data; (2) wrap the function into a DB transaction which can be
    /// atomically rolled back on error or committed on success.
    fn process_dlc_message(&self, node_id: PublicKey, msg: &Message) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %dlc_message_name(msg),
            "Processing message"
        );

        let resp = match msg {
            Message::OnChain(_) | Message::SubChannel(_) => {
                tracing::warn!(from = %node_id, kind = %dlc_message_name(msg),"Ignoring unexpected dlc message.");
                None
            }
            Message::Channel(channel_msg) => {
                let protocol_id = match channel_msg.get_reference_id() {
                    Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
                    None => None,
                };

                tracing::debug!(
                    from = %node_id,
                    ?protocol_id,
                    "Received channel message"
                );

                let inbound_msg = {
                    let mut conn = self.pool.get()?;
                    let serialized_inbound_message = SerializedDlcMessage::try_from(msg)?;
                    let inbound_msg = DlcMessage::new(node_id, serialized_inbound_message, true)?;
                    match db::dlc_messages::get(&mut conn, &inbound_msg.message_hash)? {
                        Some(_) => {
                            tracing::debug!(%node_id, kind=%dlc_message_name(msg), "Received message that has already been processed, skipping.");
                            return Ok(());
                        }
                        None => inbound_msg,
                    }
                };

                let resp = self
                    .inner
                    .dlc_manager
                    .on_dlc_message(msg, to_secp_pk_29(node_id))
                    .with_context(|| {
                        format!(
                            "Failed to handle {} dlc message from {node_id}",
                            dlc_message_name(msg)
                        )
                    })?;

                {
                    let mut conn = self.pool.get()?;
                    db::dlc_messages::insert(&mut conn, inbound_msg)?;
                }

                match channel_msg {
                    ChannelMessage::RenewFinalize(RenewFinalize {
                        channel_id,
                        reference_id,
                        ..
                    }) => {
                        // TODO: Receiving this message used to be specific to rolling over, but we
                        // now use the renew protocol for all (non-closing)
                        // trades beyond the first one.

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
                        let contract_id =
                            channel.get_contract_id().context("missing contract id")?;

                        let protocol_executor =
                            dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                        if self.is_in_rollover(node_id)? {
                            protocol_executor.finish_rollover_dlc_protocol(
                                protocol_id,
                                &contract_id,
                                &channel.get_id(),
                                &to_secp_pk_30(channel.get_counter_party_id()),
                            )?;
                        } else {
                            protocol_executor.finish_trade_dlc_protocol(
                                protocol_id,
                                false,
                                &contract_id,
                                &channel.get_id(),
                            )?;
                        }
                    }
                    ChannelMessage::SettleFinalize(SettleFinalize {
                        channel_id,
                        reference_id,
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

                        let mut connection = self.pool.get()?;
                        let dlc_protocol =
                            db::dlc_protocols::get_dlc_protocol(&mut connection, protocol_id)?;

                        let trader_id = dlc_protocol.trader.to_string();
                        tracing::debug!(trader_id, ?protocol_id, "Finalize closing position",);

                        let contract_id = dlc_protocol.contract_id;

                        match self.inner.get_closed_contract(contract_id) {
                            Ok(Some(closed_contract)) => closed_contract,
                            Ok(None) => {
                                tracing::error!(
                                    trader_id,
                                    ?protocol_id,
                                    "Can't close position as contract is not closed."
                                );
                                bail!("Can't close position as contract is not closed.");
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to get closed contract from DLC manager storage: {e:#}"
                                );
                                bail!(e);
                            }
                        };

                        let protocol_executor =
                            dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                        protocol_executor.finish_trade_dlc_protocol(
                            protocol_id,
                            true,
                            &contract_id,
                            channel_id,
                        )?;
                    }
                    ChannelMessage::CollaborativeCloseOffer(close_offer) => {
                        tracing::info!(
                            channel_id = hex::encode(close_offer.channel_id),
                            node_id = node_id.to_string(),
                            "Received an offer to collaboratively close a channel"
                        );

                        // TODO(bonomat): we should verify that the proposed amount is acceptable
                        self.inner
                            .accept_dlc_channel_collaborative_close(&close_offer.channel_id)?;
                    }
                    ChannelMessage::Accept(AcceptChannel {
                        temporary_channel_id,
                        reference_id,
                        ..
                    }) => {
                        let channel_id = match resp {
                            Some(Message::Channel(ChannelMessage::Sign(SignChannel {
                                channel_id,
                                ..
                            }))) => channel_id,
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
                        let contract_id =
                            channel.get_contract_id().context("missing contract id")?;

                        let protocol_executor =
                            dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
                        protocol_executor.finish_trade_dlc_protocol(
                            protocol_id,
                            false,
                            &contract_id,
                            &channel_id,
                        )?;
                    }
                    ChannelMessage::Reject(reject) => {
                        // TODO(holzeis): if an dlc channel gets rejected we have to deal with the
                        // counterparty as well.

                        let channel_id_hex_string = hex::encode(reject.channel_id);

                        let channel = self.inner.get_dlc_channel_by_id(&reject.channel_id)?;
                        let mut connection = self.pool.get()?;

                        match channel {
                            Channel::Cancelled(_) => {
                                tracing::info!(
                                    channel_id = channel_id_hex_string,
                                    node_id = node_id.to_string(),
                                    "DLC Channel offer has been rejected. Setting position to failed."
                                );

                                db::positions::Position::update_proposed_position(
                                    &mut connection,
                                    node_id.to_string(),
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

                                db::positions::Position::update_proposed_position(
                                    &mut connection,
                                    node_id.to_string(),
                                    PositionState::Failed,
                                )?;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                };

                resp
            }
        };

        if let Some(msg) = resp {
            tracing::info!(
                to = %node_id,
                kind = %dlc_message_name(&msg),
                "Sending message"
            );

            self.inner
                .event_handler
                .publish(NodeEvent::SendDlcMessage { peer: node_id, msg })?;
        }

        Ok(())
    }
}
