use crate::db;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::storage::TenTenOneNodeStorage;
use crate::trade::order;
use crate::trade::order::FailureReason;
use crate::trade::order::InvalidSubchannelOffer;
use crate::trade::position;
use crate::trade::position::handler::update_position_after_dlc_channel_creation_or_update;
use crate::trade::position::handler::update_position_after_dlc_closure;
use crate::trade::position::PositionState;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::ReferenceId;
use dlc_messages::channel::CollaborativeCloseOffer;
use dlc_messages::channel::OfferChannel;
use dlc_messages::channel::Reject;
use dlc_messages::channel::RenewOffer;
use dlc_messages::channel::SettleOffer;
use lightning::chain::transaction::OutPoint;
use lightning::sign::DelayedPaymentOutputDescriptor;
use lightning::sign::SpendableOutputDescriptor;
use lightning::sign::StaticPaymentOutputDescriptor;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::task::JoinHandle;
use tracing::instrument;
use uuid::Uuid;
use xxi_node::bitcoin_conversion::to_secp_pk_30;
use xxi_node::commons;
use xxi_node::commons::OrderReason;
use xxi_node::dlc_message::DlcMessage;
use xxi_node::dlc_message::SerializedDlcMessage;
use xxi_node::message_handler::TenTenOneAcceptChannel;
use xxi_node::message_handler::TenTenOneCollaborativeCloseOffer;
use xxi_node::message_handler::TenTenOneMessage;
use xxi_node::message_handler::TenTenOneMessageType;
use xxi_node::message_handler::TenTenOneOfferChannel;
use xxi_node::message_handler::TenTenOneReject;
use xxi_node::message_handler::TenTenOneRenewAccept;
use xxi_node::message_handler::TenTenOneRenewOffer;
use xxi_node::message_handler::TenTenOneRenewRevoke;
use xxi_node::message_handler::TenTenOneRolloverAccept;
use xxi_node::message_handler::TenTenOneRolloverOffer;
use xxi_node::message_handler::TenTenOneRolloverRevoke;
use xxi_node::message_handler::TenTenOneSettleConfirm;
use xxi_node::message_handler::TenTenOneSettleOffer;
use xxi_node::message_handler::TenTenOneSignChannel;
use xxi_node::node;
use xxi_node::node::event::NodeEvent;
use xxi_node::node::rust_dlc_manager::DlcChannelId;
use xxi_node::node::tentenone_message_name;
use xxi_node::node::NodeInfo;
use xxi_node::node::RunningNode;
use xxi_node::transaction::Transaction;
use xxi_node::TransactionDetails;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<
        node::Node<
            bdk_file_store::Store<bdk::wallet::ChangeSet>,
            TenTenOneNodeStorage,
            NodeStorage,
        >,
    >,
    _running: Arc<RunningNode>,
    // TODO: we should make this persistent as invoices might get paid later - but for now this is
    // good enough
    pub pending_usdp_invoices: Arc<parking_lot::Mutex<HashSet<bitcoin_old::hashes::sha256::Hash>>>,

    pub watcher_handle: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
}

impl Node {
    pub fn new(
        node: Arc<
            node::Node<
                bdk_file_store::Store<bdk::wallet::ChangeSet>,
                TenTenOneNodeStorage,
                NodeStorage,
            >,
        >,
        running: RunningNode,
    ) -> Self {
        Self {
            inner: node,
            _running: Arc::new(running),
            pending_usdp_invoices: Arc::new(Default::default()),
            watcher_handle: Arc::new(Default::default()),
        }
    }
}

pub struct Balances {
    pub on_chain: u64,
    pub off_chain: Option<u64>,
}

impl From<Balances> for crate::event::api::Balances {
    fn from(value: Balances) -> Self {
        Self {
            on_chain: value.on_chain,
            off_chain: value.off_chain,
        }
    }
}

pub struct WalletHistory {
    pub on_chain: Vec<TransactionDetails>,
}

impl Node {
    pub fn get_blockchain_height(&self) -> Result<u64> {
        self.inner.get_blockchain_height()
    }

    pub fn get_wallet_balances(&self) -> Balances {
        let on_chain = self.inner.get_on_chain_balance();
        let on_chain = on_chain.confirmed + on_chain.trusted_pending;

        let off_chain = match self.inner.get_dlc_channels_usable_balance() {
            Ok(off_chain) => Some(off_chain.to_sat()),
            Err(e) => {
                tracing::error!("Failed to get dlc channels usable balance. {e:#}");
                None
            }
        };

        Balances {
            on_chain,
            off_chain,
        }
    }

    pub fn get_wallet_history(&self) -> WalletHistory {
        let on_chain = self.inner.get_on_chain_history();

        WalletHistory { on_chain }
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
            let msg_type = msg.get_tentenone_message_type();
            if let Err(e) = self.process_dlc_message(to_secp_pk_30(node_id), msg) {
                tracing::error!(
                    from = %node_id,
                    kind = %msg_name,
                    "Failed to process incoming DLC message: {e:#}"
                );

                let task_status = TaskStatus::Failed(format!("{e:#}"));
                let task = match msg_type {
                    TenTenOneMessageType::Trade => BackgroundTask::AsyncTrade(task_status),
                    TenTenOneMessageType::Expire => BackgroundTask::Expire(task_status),
                    TenTenOneMessageType::Liquidate => BackgroundTask::Liquidate(task_status),
                    TenTenOneMessageType::Rollover => BackgroundTask::Rollover(task_status),
                    TenTenOneMessageType::Other => {
                        tracing::warn!("Ignoring error received from coordinator unrelated to a trade or rollover.");
                        continue;
                    }
                };
                event::publish(&EventInternal::BackgroundNotification(task));
            }
        }
    }

    /// [`process_dlc_message`] processes incoming dlc messages and updates the 10101
    /// position accordingly.
    /// - Any other message will be ignored.
    /// - Any dlc message that has already been processed will be skipped.
    ///
    /// If an offer is received [`TenTenOneMessage::Offer`], [`TenTenOneMessage::SettleOffer`],
    /// [`TenTenOneMessage::CollaborativeCloseOffer`], [`TenTenOneMessage::RenewOffer`] will get
    /// automatically accepted. Unless the maturity date of the offer is already outdated.
    ///
    /// FIXME(holzeis): This function manipulates different data objects in different data sources
    /// and should use a transaction to make all changes atomic. Not doing so risks of ending up in
    /// an inconsistent state. One way of fixing that could be to
    /// (1) use a single data source for the 10101 data and the rust-dlc data.
    /// (2) wrap the function into a db transaction which can be atomically rolled back on error or
    /// committed on success.
    fn process_dlc_message(&self, node_id: PublicKey, msg: TenTenOneMessage) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %tentenone_message_name(&msg),
            "Processing message"
        );

        tracing::debug!(
            from = %node_id,
            "Received message"
        );

        let inbound_msg = {
            let mut conn = db::connection()?;
            let serialized_inbound_message = SerializedDlcMessage::try_from(&msg)?;
            let inbound_msg = DlcMessage::new(node_id, serialized_inbound_message, true)?;
            match db::dlc_messages::DlcMessage::get(&mut conn, &inbound_msg.message_hash)? {
                Some(_) => {
                    tracing::debug!(%node_id, kind=%tentenone_message_name(&msg), "Received message that has already been processed, skipping.");
                    return Ok(());
                }
                None => inbound_msg,
            }
        };

        let resp = match self
            .inner
            .process_tentenone_message(msg.clone(), node_id)
            .with_context(|| {
                format!(
                    "Failed to handle {} message from {node_id}",
                    tentenone_message_name(&msg)
                )
            }) {
            Ok(resp) => resp,
            Err(e) => {
                match &msg {
                    TenTenOneMessage::Offer(TenTenOneOfferChannel {
                        offer_channel:
                            OfferChannel {
                                temporary_channel_id: channel_id,
                                reference_id,
                                ..
                            },
                        ..
                    })
                    | TenTenOneMessage::SettleOffer(TenTenOneSettleOffer {
                        settle_offer:
                            SettleOffer {
                                channel_id,
                                reference_id,
                                ..
                            },
                        ..
                    })
                    | TenTenOneMessage::RenewOffer(TenTenOneRenewOffer {
                        renew_offer:
                            RenewOffer {
                                channel_id,
                                reference_id,
                                ..
                            },
                        ..
                    })
                    | TenTenOneMessage::RolloverOffer(TenTenOneRolloverOffer {
                        renew_offer:
                            RenewOffer {
                                channel_id,
                                reference_id,
                                ..
                            },
                        ..
                    }) => {
                        if let Err(e) = self.force_reject_offer(node_id, *channel_id, *reference_id)
                        {
                            tracing::error!(
                                channel_id = hex::encode(channel_id),
                                "Failed to reject offer. Error: {e:#}"
                            );
                        }
                    }
                    _ => {}
                }

                return Err(e);
            }
        };

        if let Some(msg) = resp.clone() {
            // store dlc message immediately so we do not lose the response if something
            // goes wrong afterwards.
            self.inner
                .event_handler
                .publish(NodeEvent::StoreDlcMessage { peer: node_id, msg });
        }

        {
            let mut conn = db::connection()?;
            db::dlc_messages::DlcMessage::insert(&mut conn, inbound_msg)?;
        }

        match msg {
            TenTenOneMessage::Offer(offer) => {
                tracing::info!(
                    channel_id = hex::encode(offer.offer_channel.temporary_channel_id),
                    "Automatically accepting dlc channel offer"
                );
                self.process_dlc_channel_offer(&offer)?;
            }
            TenTenOneMessage::SettleOffer(offer) => {
                tracing::info!(
                    channel_id = hex::encode(offer.settle_offer.channel_id),
                    "Automatically accepting settle offer"
                );
                self.process_settle_offer(&offer)?;
            }
            TenTenOneMessage::RenewOffer(offer) => {
                tracing::info!(
                    channel_id = hex::encode(offer.renew_offer.channel_id),
                    "Automatically accepting renew offer"
                );

                self.process_renew_offer(&offer)?;
            }
            TenTenOneMessage::RolloverOffer(offer) => {
                tracing::info!(
                    channel_id = hex::encode(offer.renew_offer.channel_id),
                    "Automatically accepting rollover offer"
                );

                self.process_rollover_offer(&offer)?;
            }
            TenTenOneMessage::RenewRevoke(TenTenOneRenewRevoke {
                renew_revoke,
                order_id,
            }) => {
                let channel_id_hex = hex::encode(renew_revoke.channel_id);

                tracing::info!(
                    order_id = %order_id,
                    channel_id = %channel_id_hex,
                    "Finished renew protocol"
                );

                let expiry_timestamp = self
                    .inner
                    .get_expiry_for_confirmed_dlc_channel(&renew_revoke.channel_id)?;

                let filled_order = order::handler::order_filled(Some(order_id))
                    .context("Cannot mark order as filled for confirmed DLC")?;

                update_position_after_dlc_channel_creation_or_update(
                    filled_order,
                    expiry_timestamp,
                )
                .context("Failed to update position after DLC update")?;

                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::AsyncTrade(TaskStatus::Success),
                ));
            }
            TenTenOneMessage::RolloverRevoke(TenTenOneRolloverRevoke { renew_revoke }) => {
                let channel_id_hex = hex::encode(renew_revoke.channel_id);

                tracing::info!(
                    channel_id = %channel_id_hex,
                    "Finished rollover protocol"
                );

                position::handler::set_position_state(PositionState::Open)?;

                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::Rollover(TaskStatus::Success),
                ));
            }
            TenTenOneMessage::Sign(TenTenOneSignChannel {
                order_id,
                sign_channel,
            }) => {
                let expiry_timestamp = self
                    .inner
                    .get_expiry_for_confirmed_dlc_channel(&sign_channel.channel_id)?;

                let filled_order = order::handler::order_filled(Some(order_id))
                    .context("Cannot mark order as filled for confirmed DLC")?;

                update_position_after_dlc_channel_creation_or_update(
                    filled_order,
                    expiry_timestamp,
                )
                .context("Failed to update position after DLC creation")?;

                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::AsyncTrade(TaskStatus::Success),
                ));
            }
            TenTenOneMessage::SettleConfirm(TenTenOneSettleConfirm { order_id, .. }) => {
                tracing::debug!("Position based on DLC channel is being closed");

                let filled_order = order::handler::order_filled(Some(order_id))?;

                update_position_after_dlc_closure(filled_order.clone())
                    .context("Failed to update position after DLC closure")?;

                let task = match filled_order.reason.into() {
                    OrderReason::Manual => BackgroundTask::AsyncTrade(TaskStatus::Success),
                    OrderReason::Expired => BackgroundTask::Expire(TaskStatus::Success),
                    OrderReason::CoordinatorLiquidated | OrderReason::TraderLiquidated => {
                        BackgroundTask::Liquidate(TaskStatus::Success)
                    }
                };
                event::publish(&EventInternal::BackgroundNotification(task));
            }
            TenTenOneMessage::CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer {
                collaborative_close_offer: CollaborativeCloseOffer { channel_id, .. },
            }) => {
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::CloseChannel(TaskStatus::Pending),
                ));

                let channel_id_hex_string = hex::encode(channel_id);
                tracing::info!(
                    channel_id = channel_id_hex_string,
                    node_id = node_id.to_string(),
                    "Received an offer to collaboratively close a channel"
                );

                // TODO(bonomat): we should verify that the proposed amount is acceptable
                self.inner
                    .accept_dlc_channel_collaborative_close(&channel_id)
                    .inspect_err(|e| {
                        event::publish(&EventInternal::BackgroundNotification(
                            BackgroundTask::CloseChannel(TaskStatus::Failed(format!("{e:#}"))),
                        ))
                    })?;

                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::CloseChannel(TaskStatus::Success),
                ));
            }
            _ => (),
        }

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

    /// Rejects a offer that failed to get processed during the [`dlc_manager.on_dlc_message`].
    ///
    /// This function will simply update the 10101 meta data and send the reject message without
    /// going through rust-dlc.
    ///
    /// Note we can't use the rust-dlc api to reject the offer as the processing failed
    /// and the `Channel::Offered`, `Channel::RenewOffered`, `Channel::SettledOffered`  have not
    /// been stored to the dlc store. The corresponding reject offer would fail in rust-dlc,
    /// because this the expected channel can't be found by the provided `channel_id`.
    #[instrument(fields(channel_id = hex::encode(channel_id), %counterparty),skip_all, err(Debug))]
    pub fn force_reject_offer(
        &self,
        counterparty: PublicKey,
        channel_id: DlcChannelId,
        reference_id: Option<ReferenceId>,
    ) -> Result<()> {
        let now = std::time::SystemTime::now();
        let now = now
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Unexpected time error")
            .as_secs();

        let reject = Reject {
            channel_id,
            timestamp: now,
            reference_id,
        };

        order::handler::order_failed(
            None,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept offer"),
        )
        .context("Could not set order to failed")?;

        self.send_dlc_message(
            counterparty,
            TenTenOneMessage::Reject(TenTenOneReject { reject }),
        )
    }

    #[instrument(fields(channel_id = hex::encode(channel_id)),skip_all, err(Debug))]
    pub fn reject_dlc_channel_offer(
        &self,
        order_id: Option<Uuid>,
        channel_id: &DlcChannelId,
    ) -> Result<()> {
        tracing::warn!("Rejecting dlc channel offer!");

        let (reject, counterparty) = self
            .inner
            .dlc_manager
            .reject_channel(channel_id)
            .with_context(|| {
                format!(
                    "Failed to reject DLC channel offer for channel {}",
                    hex::encode(channel_id)
                )
            })?;

        order::handler::order_failed(
            order_id,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept dlc channel offer"),
        )
        .context("Could not set order to failed")?;

        self.send_dlc_message(
            to_secp_pk_30(counterparty),
            TenTenOneMessage::Reject(TenTenOneReject { reject }),
        )
    }

    #[instrument(fields(channel_id = hex::encode(offer.offer_channel.temporary_channel_id)),skip_all, err(Debug))]
    pub fn process_dlc_channel_offer(&self, offer: &TenTenOneOfferChannel) -> Result<()> {
        // TODO(holzeis): We should check if the offered amounts are expected.
        self.set_order_to_filling(offer.filled_with.clone())?;
        let order_id = offer.filled_with.order_id;

        let channel_id = offer.offer_channel.temporary_channel_id;
        match self
            .inner
            .dlc_manager
            .accept_channel(
                &channel_id,
                offer
                    .offer_channel
                    .fee_config
                    .map(dlc::FeeConfig::from)
                    .unwrap_or(dlc::FeeConfig::EvenSplit),
            )
            .map_err(anyhow::Error::new)
        {
            Ok((accept_channel, _, _, node_id)) => {
                self.send_dlc_message(
                    to_secp_pk_30(node_id),
                    TenTenOneMessage::Accept(TenTenOneAcceptChannel {
                        accept_channel,
                        order_id: offer.filled_with.order_id,
                    }),
                )?;
            }
            Err(e) => {
                tracing::error!("Failed to accept DLC channel offer: {e:#}");
                self.reject_dlc_channel_offer(Some(order_id), &channel_id)?;
            }
        }

        Ok(())
    }

    #[instrument(fields(channel_id = hex::encode(channel_id)),skip_all, err(Debug))]
    pub fn reject_settle_offer(
        &self,
        order_id: Option<Uuid>,
        channel_id: &DlcChannelId,
    ) -> Result<()> {
        tracing::warn!("Rejecting pending dlc channel collaborative settlement offer!");
        let (reject, counterparty) = self.inner.dlc_manager.reject_settle_offer(channel_id)?;

        order::handler::order_failed(
            order_id,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept settle offer"),
        )?;

        self.send_dlc_message(
            to_secp_pk_30(counterparty),
            TenTenOneMessage::Reject(TenTenOneReject { reject }),
        )
    }

    #[instrument(fields(channel_id = hex::encode(offer.settle_offer.channel_id)),skip_all, err(Debug))]
    pub fn process_settle_offer(&self, offer: &TenTenOneSettleOffer) -> Result<()> {
        // TODO(holzeis): We should check if the offered amounts are expected.
        let order_reason = offer.order.clone().order_reason;
        let order_id = offer.order.id;

        match order_reason {
            OrderReason::Expired
            | OrderReason::CoordinatorLiquidated
            | OrderReason::TraderLiquidated => {
                tracing::info!(
                    %order_id,
                    "Received an async match from orderbook. Reason: {order_reason:?}"
                );

                let task = if order_reason == OrderReason::Expired {
                    BackgroundTask::Expire(TaskStatus::Pending)
                } else {
                    BackgroundTask::Liquidate(TaskStatus::Pending)
                };

                event::publish(&EventInternal::BackgroundNotification(task));

                order::handler::async_order_filling(&offer.order, &offer.filled_with)
                    .with_context(||
                        format!("Failed to process async match update from orderbook. order_id {order_id}"))?;
            }
            // Received a regular settle offer after a manual order.
            //
            // TODO(holzeis): Eventually this should as well start the trade dialog. At the moment
            // we automatically show the trade dialog since we expect a synchronous response from
            // the orderbook.
            OrderReason::Manual => self.set_order_to_filling(offer.filled_with.clone())?,
        }

        let order_id = offer.order.id;
        let channel_id = offer.settle_offer.channel_id;
        if let Err(e) = self.inner.accept_dlc_channel_collaborative_settlement(
            offer.filled_with.order_id,
            order_reason,
            &channel_id,
        ) {
            tracing::error!("Failed to accept dlc channel collaborative settlement offer. {e:#}");
            self.reject_settle_offer(Some(order_id), &channel_id)?;
        }

        Ok(())
    }

    fn set_order_to_filling(&self, filled_with: commons::FilledWith) -> Result<()> {
        let order_id = filled_with.order_id;
        tracing::info!(%order_id, "Received match from orderbook");
        let execution_price = filled_with
            .average_execution_price()
            .to_f32()
            .expect("to fit into f32");

        let matching_fee = filled_with.order_matching_fee();

        order::handler::order_filling(order_id, execution_price, matching_fee).with_context(|| {
            format!("Failed to process match update from orderbook. order_id = {order_id}")
        })
    }

    #[instrument(fields(channel_id = hex::encode(channel_id)),skip_all, err(Debug))]
    pub fn reject_renew_offer(
        &self,
        order_id: Option<Uuid>,
        channel_id: &DlcChannelId,
    ) -> Result<()> {
        tracing::warn!("Rejecting dlc channel renew offer!");

        let (reject, counterparty) = self.inner.dlc_manager.reject_renew_offer(channel_id)?;

        order::handler::order_failed(
            order_id,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept renew offer"),
        )?;

        self.send_dlc_message(
            to_secp_pk_30(counterparty),
            TenTenOneMessage::Reject(TenTenOneReject { reject }),
        )
    }

    #[instrument(fields(channel_id = hex::encode(offer.renew_offer.channel_id)),skip_all, err(Debug))]
    pub fn process_renew_offer(&self, offer: &TenTenOneRenewOffer) -> Result<()> {
        // TODO(holzeis): We should check if the offered amounts are expected.
        let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
            offer.renew_offer.contract_info.get_closest_maturity_date() as i64,
        )?;

        let order_id = offer.filled_with.order_id;
        self.set_order_to_filling(offer.filled_with.clone())?;

        let channel_id = offer.renew_offer.channel_id;
        match self.inner.dlc_manager.accept_renew_offer(&channel_id) {
            Ok((renew_accept, node_id)) => {
                position::handler::handle_channel_renewal_offer(expiry_timestamp)?;

                self.send_dlc_message(
                    to_secp_pk_30(node_id),
                    TenTenOneMessage::RenewAccept(TenTenOneRenewAccept {
                        renew_accept,
                        order_id: offer.filled_with.order_id,
                    }),
                )?;
            }
            Err(e) => {
                tracing::error!("Failed to accept dlc channel renew offer. {e:#}");

                self.reject_renew_offer(Some(order_id), &channel_id)?;
            }
        };

        Ok(())
    }

    #[instrument(fields(channel_id = hex::encode(channel_id)),skip_all, err(Debug))]
    pub fn reject_rollover_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        tracing::warn!("Rejecting rollover offer!");

        let (reject, counterparty) = self.inner.dlc_manager.reject_renew_offer(channel_id)?;

        self.send_dlc_message(
            to_secp_pk_30(counterparty),
            TenTenOneMessage::Reject(TenTenOneReject { reject }),
        )
    }

    #[instrument(fields(channel_id = hex::encode(offer.renew_offer.channel_id)),skip_all, err(Debug))]
    pub fn process_rollover_offer(&self, offer: &TenTenOneRolloverOffer) -> Result<()> {
        tracing::info!("Received a rollover notification from orderbook.");
        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::Rollover(TaskStatus::Pending),
        ));

        let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
            offer.renew_offer.contract_info.get_closest_maturity_date() as i64,
        )?;

        let channel_id = offer.renew_offer.channel_id;
        match self.inner.dlc_manager.accept_renew_offer(&channel_id) {
            Ok((renew_accept, node_id)) => {
                position::handler::handle_channel_renewal_offer(expiry_timestamp)?;

                self.send_dlc_message(
                    to_secp_pk_30(node_id),
                    TenTenOneMessage::RolloverAccept(TenTenOneRolloverAccept { renew_accept }),
                )?;
            }
            Err(e) => {
                tracing::error!("Failed to accept dlc channel rollover offer. {e:#}");
                event::publish(&EventInternal::BackgroundNotification(
                    BackgroundTask::Rollover(TaskStatus::Failed(format!("{e:#}"))),
                ));

                self.reject_rollover_offer(&channel_id)?;
            }
        };

        Ok(())
    }

    pub fn send_dlc_message(&self, node_id: PublicKey, msg: TenTenOneMessage) -> Result<()> {
        tracing::info!(
            to = %node_id,
            kind = %tentenone_message_name(&msg),
            "Sending message"
        );

        self.inner.event_handler.publish(NodeEvent::SendDlcMessage {
            peer: node_id,
            msg: msg.clone(),
        });

        Ok(())
    }

    pub async fn keep_connected(&self, peer: NodeInfo) {
        let reconnect_interval = Duration::from_secs(1);
        loop {
            let connection_closed_future = match self.inner.connect(peer).await {
                Ok(fut) => fut,
                Err(e) => {
                    tracing::warn!(
                        %peer,
                        ?reconnect_interval,
                        "Connection failed: {e:#}; reconnecting"
                    );

                    tokio::time::sleep(reconnect_interval).await;
                    continue;
                }
            };

            connection_closed_future.await;
            tracing::debug!(
                %peer,
                ?reconnect_interval,
                "Connection lost; reconnecting"
            );

            tokio::time::sleep(reconnect_interval).await;
        }
    }
}

#[derive(Clone)]
pub struct NodeStorage;

impl node::Storage for NodeStorage {
    // Spendable outputs

    fn insert_spendable_output(&self, descriptor: SpendableOutputDescriptor) -> Result<()> {
        use SpendableOutputDescriptor::*;
        let outpoint = match &descriptor {
            // Static outputs don't need to be persisted because they pay directly to an address
            // owned by the on-chain wallet
            StaticOutput { .. } => return Ok(()),
            DelayedPaymentOutput(DelayedPaymentOutputDescriptor { outpoint, .. }) => outpoint,
            StaticPaymentOutput(StaticPaymentOutputDescriptor { outpoint, .. }) => outpoint,
        };

        db::insert_spendable_output(*outpoint, descriptor)
    }

    fn get_spendable_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<SpendableOutputDescriptor>> {
        db::get_spendable_output(*outpoint)
    }

    fn delete_spendable_output(&self, outpoint: &OutPoint) -> Result<()> {
        db::delete_spendable_output(*outpoint)
    }

    fn all_spendable_outputs(&self) -> Result<Vec<SpendableOutputDescriptor>> {
        db::get_spendable_outputs()
    }

    // Transactions

    fn upsert_transaction(&self, transaction: Transaction) -> Result<()> {
        db::upsert_transaction(transaction)
    }

    fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>> {
        db::get_transaction(txid)
    }

    fn all_transactions_without_fees(&self) -> Result<Vec<Transaction>> {
        db::get_all_transactions_without_fees()
    }
}
