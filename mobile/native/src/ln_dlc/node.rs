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
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::TransactionDetails;
use bitcoin::hashes::hex::ToHex;
use bitcoin::Txid;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use lightning::chain::transaction::OutPoint;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning::sign::DelayedPaymentOutputDescriptor;
use lightning::sign::SpendableOutputDescriptor;
use lightning::sign::StaticPaymentOutputDescriptor;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::dlc_message::DlcMessage;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::rust_dlc_manager::DlcChannelId;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::PaymentDetails;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::transaction::Transaction;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::MillisatAmount;
use ln_dlc_node::PaymentFlow;
use ln_dlc_node::PaymentInfo;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tracing::instrument;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<node::Node<TenTenOneNodeStorage, NodeStorage>>,
    _running: Arc<RunningNode>,
    // TODO: we should make this persistent as invoices might get paid later - but for now this is
    // good enough
    pub pending_usdp_invoices: Arc<parking_lot::Mutex<HashSet<bitcoin::hashes::sha256::Hash>>>,
}

impl Node {
    pub fn new(
        node: Arc<node::Node<TenTenOneNodeStorage, NodeStorage>>,
        running: RunningNode,
    ) -> Self {
        Self {
            inner: node,
            _running: Arc::new(running),
            pending_usdp_invoices: Arc::new(Default::default()),
        }
    }
}

pub struct Balances {
    pub on_chain: u64,
    pub off_chain: u64,
}

impl From<Balances> for crate::api::Balances {
    fn from(value: Balances) -> Self {
        Self {
            on_chain: value.on_chain,
            off_chain: value.off_chain,
        }
    }
}

pub struct WalletHistories {
    pub on_chain: Vec<TransactionDetails>,
    pub off_chain: Vec<PaymentDetails>,
}

impl Node {
    pub fn get_blockchain_height(&self) -> Result<u64> {
        self.inner.get_blockchain_height()
    }

    pub fn get_wallet_balances(&self) -> Balances {
        let on_chain = match self.inner.get_on_chain_balance() {
            Ok(on_chain) => on_chain.confirmed + on_chain.trusted_pending,
            Err(e) => {
                tracing::error!("Failed to get onchain balance. {e:#}");
                0
            }
        };

        let off_chain = match self.inner.get_dlc_channels_usable_balance() {
            Ok(off_chain) => off_chain.to_sat(),
            Err(e) => {
                tracing::error!("Failed to get dlc channels usable balance. {e:#}");
                0
            }
        };

        Balances {
            on_chain,
            off_chain,
        }
    }

    pub fn get_wallet_histories(&self) -> Result<WalletHistories> {
        let on_chain = self.inner.get_on_chain_history()?;
        let off_chain = self.inner.get_off_chain_history()?;

        Ok(WalletHistories {
            on_chain,
            off_chain,
        })
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
            if let Err(e) = self.process_dlc_message(node_id, msg) {
                tracing::error!(
                    from = %node_id,
                    kind = %msg_name,
                    "Failed to process incoming DLC message: {e:#}"
                );
            }
        }
    }

    /// [`process_dlc_message`] processes incoming dlc channel messages and updates the 10101
    /// position accordingly.
    /// - Any other message will be ignored.
    /// - Any dlc channel message that has already been processed will be skipped.
    ///
    /// If an offer is received [`ChannelMessage::Offer`], [`ChannelMessage::SettleOffer`],
    /// [`ChannelMessage::CollaborativeCloseOffer`], [`ChannelMessage::RenewOffer`] will get
    /// automatically accepted. Unless the maturity date of the offer is already outdated.
    ///
    /// FIXME(holzeis): This function manipulates different data objects in different data sources
    /// and should use a transaction to make all changes atomic. Not doing so risks of ending up in
    /// an inconsistent state. One way of fixing that could be to
    /// (1) use a single data source for the 10101 data and the rust-dlc data.
    /// (2) wrap the function into a db transaction which can be atomically rolled back on error or
    /// committed on success.
    fn process_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %dlc_message_name(&msg),
            "Processing message"
        );

        let resp = match &msg {
            Message::OnChain(_) | Message::SubChannel(_) => {
                tracing::warn!("Ignoring unexpected dlc message.");
                None
            }
            Message::Channel(channel_msg) => {
                let inbound_msg = {
                    let mut conn = db::connection()?;
                    let serialized_inbound_message = SerializedDlcMessage::try_from(&msg)?;
                    let inbound_msg = DlcMessage::new(node_id, serialized_inbound_message, true)?;
                    match db::dlc_messages::DlcMessage::get(&mut conn, &inbound_msg.message_hash)? {
                        Some(_) => {
                            tracing::debug!(%node_id, kind=%dlc_message_name(&msg), "Received message that has already been processed, skipping.");
                            return Ok(());
                        }
                        None => inbound_msg,
                    }
                };

                let resp = self
                    .inner
                    .dlc_manager
                    .on_dlc_message(&msg, node_id)
                    .with_context(|| {
                        format!(
                            "Failed to handle {} message from {node_id}",
                            dlc_message_name(&msg)
                        )
                    })?;

                {
                    let mut conn = db::connection()?;
                    db::dlc_messages::DlcMessage::insert(&mut conn, inbound_msg)?;
                }

                match channel_msg {
                    ChannelMessage::Offer(offer) => {
                        tracing::info!(
                            channel_id = offer.temporary_channel_id.to_hex(),
                            "Automatically accepting dlc channel offer"
                        );
                        self.process_dlc_channel_offer(&offer.temporary_channel_id)?;
                    }
                    ChannelMessage::SettleOffer(offer) => {
                        tracing::info!(
                            channel_id = offer.channel_id.to_hex(),
                            "Automatically accepting settle offer"
                        );
                        self.process_settle_offer(&offer.channel_id)?;
                    }
                    ChannelMessage::RenewOffer(r) => {
                        tracing::info!(
                            channel_id = r.channel_id.to_hex(),
                            "Automatically accepting renew offer"
                        );

                        let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
                            r.contract_info.get_closest_maturity_date() as i64,
                        )?;
                        self.process_renew_offer(&r.channel_id, expiry_timestamp)?;
                    }
                    ChannelMessage::RenewRevoke(r) => {
                        let channel_id_hex = r.channel_id.to_hex();

                        tracing::info!(
                            channel_id = %channel_id_hex,
                            "Finished renew protocol"
                        );

                        let expiry_timestamp = self
                            .inner
                            .get_expiry_for_confirmed_dlc_channel(&r.channel_id)?;

                        match db::get_order_in_filling()? {
                            Some(_) => {
                                let filled_order = order::handler::order_filled()
                                    .context("Cannot mark order as filled for confirmed DLC")?;

                                update_position_after_dlc_channel_creation_or_update(
                                    filled_order,
                                    expiry_timestamp,
                                )
                                .context("Failed to update position after DLC creation")?;
                            }
                            // If there is no order in `Filling` we must be rolling over.
                            None => {
                                tracing::info!(
                                    channel_id = %channel_id_hex,
                                    "Finished rolling over position"
                                );

                                position::handler::set_position_state(PositionState::Open)?;

                                event::publish(&EventInternal::BackgroundNotification(
                                    BackgroundTask::Rollover(TaskStatus::Success),
                                ));
                            }
                        };
                    }
                    ChannelMessage::Sign(signed) => {
                        let expiry_timestamp = self
                            .inner
                            .get_expiry_for_confirmed_dlc_channel(&signed.channel_id)?;

                        let filled_order = order::handler::order_filled()
                            .context("Cannot mark order as filled for confirmed DLC")?;

                        update_position_after_dlc_channel_creation_or_update(
                            filled_order,
                            expiry_timestamp,
                        )
                        .context("Failed to update position after DLC creation")?;

                        // Sending always a recover dlc background notification success message here
                        // as we do not know if we might have reached this state after a restart.
                        // This event is only received by the UI at the moment indicating that the
                        // dialog can be closed. If the dialog is not open, this event would be
                        // simply ignored by the UI.
                        //
                        // FIXME(holzeis): We should not require that event and align the UI
                        // handling with waiting for an order execution in the happy case with
                        // waiting for an order execution after an in between restart. For now it
                        // was the easiest to go parallel to that implementation so that we don't
                        // have to touch it.
                        event::publish(&EventInternal::BackgroundNotification(
                            BackgroundTask::RecoverDlc(TaskStatus::Success),
                        ));
                    }
                    ChannelMessage::SettleConfirm(_) => {
                        tracing::debug!("Position based on DLC channel is being closed");

                        let filled_order = order::handler::order_filled()?;

                        update_position_after_dlc_closure(Some(filled_order))
                            .context("Failed to update position after DLC closure")?;

                        // In case of a restart.
                        event::publish(&EventInternal::BackgroundNotification(
                            BackgroundTask::RecoverDlc(TaskStatus::Success),
                        ));
                    }
                    ChannelMessage::CollaborativeCloseOffer(close_offer) => {
                        let channel_id_hex_string = close_offer.channel_id.to_hex();
                        tracing::info!(
                            channel_id = channel_id_hex_string,
                            node_id = node_id.to_string(),
                            "Received an offer to collaboratively close a channel"
                        );

                        // TODO(bonomat): we should verify that the proposed amount is acceptable
                        self.inner
                            .accept_dlc_channel_collaborative_close(&close_offer.channel_id)?;
                    }
                    _ => (),
                }

                resp
            }
        };

        if let Some(msg) = resp {
            self.send_dlc_message(node_id, msg.clone())?;
        }

        Ok(())
    }

    #[instrument(fields(channel_id = channel_id.to_hex()),skip_all, err(Debug))]
    pub fn reject_dlc_channel_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
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
            None,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept dlc channel offer"),
        )
        .context("Could not set order to failed")?;

        self.send_dlc_message(
            counterparty,
            Message::Channel(ChannelMessage::Reject(reject)),
        )
    }

    #[instrument(fields(channel_id = channel_id.to_hex()),skip_all, err(Debug))]
    pub fn process_dlc_channel_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        // TODO(holzeis): We should check if the offered amounts are expected.

        match self.inner.dlc_manager.accept_channel(channel_id) {
            Ok((accept_channel, _, _, node_id)) => {
                self.send_dlc_message(
                    node_id,
                    Message::Channel(ChannelMessage::Accept(accept_channel)),
                )?;
            }
            Err(e) => {
                tracing::error!("Failed to accept dlc channel offer. {e:#}");
                self.reject_dlc_channel_offer(channel_id)?;
            }
        }

        Ok(())
    }

    #[instrument(fields(channel_id = channel_id.to_hex()),skip_all, err(Debug))]
    pub fn reject_settle_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        tracing::warn!("Rejecting pending dlc channel collaborative settlement offer!");
        let (reject, counterparty) = self.inner.dlc_manager.reject_settle_offer(channel_id)?;

        order::handler::order_failed(
            None,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept settle offer"),
        )?;

        self.send_dlc_message(
            counterparty,
            Message::Channel(ChannelMessage::Reject(reject)),
        )
    }

    #[instrument(fields(channel_id = channel_id.to_hex()),skip_all, err(Debug))]
    pub fn process_settle_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        // TODO(holzeis): We should check if the offered amounts are expected.

        if let Err(e) = self
            .inner
            .accept_dlc_channel_collaborative_settlement(channel_id)
        {
            tracing::error!("Failed to accept dlc channel collaborative settlement offer. {e:#}");
            self.reject_settle_offer(channel_id)?;
        }

        Ok(())
    }

    #[instrument(fields(channel_id = channel_id.to_hex()),skip_all, err(Debug))]
    pub fn reject_renew_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        tracing::warn!("Rejecting dlc channel renew offer!");

        let (reject, counterparty) = self.inner.dlc_manager.reject_renew_offer(channel_id)?;

        order::handler::order_failed(
            None,
            FailureReason::InvalidDlcOffer(InvalidSubchannelOffer::Unacceptable),
            anyhow!("Failed to accept renew offer"),
        )?;

        self.send_dlc_message(
            counterparty,
            Message::Channel(ChannelMessage::Reject(reject)),
        )
    }

    #[instrument(fields(channel_id = channel_id.to_hex()),skip_all, err(Debug))]
    pub fn process_renew_offer(
        &self,
        channel_id: &DlcChannelId,
        expiry_timestamp: OffsetDateTime,
    ) -> Result<()> {
        // TODO(holzeis): We should check if the offered amounts are expected.

        match self.inner.dlc_manager.accept_renew_offer(channel_id) {
            Ok((renew_accept, node_id)) => {
                position::handler::handle_channel_renewal_offer(expiry_timestamp)?;

                self.send_dlc_message(
                    node_id,
                    Message::Channel(ChannelMessage::RenewAccept(renew_accept)),
                )?;
            }
            Err(e) => {
                tracing::error!("Failed to accept dlc channel renew offer. {e:#}");

                self.reject_renew_offer(channel_id)?;
            }
        };

        Ok(())
    }

    pub fn send_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            to = %node_id,
            kind = %dlc_message_name(&msg),
            "Sending message"
        );

        self.inner
            .event_handler
            .publish(NodeEvent::SendDlcMessage {
                peer: node_id,
                msg: msg.clone(),
            })?;

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
    // Payments

    fn insert_payment(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()> {
        db::insert_payment(payment_hash, info)
    }
    fn merge_payment(
        &self,
        payment_hash: &PaymentHash,
        flow: PaymentFlow,
        amt_msat: MillisatAmount,
        fee_msat: MillisatAmount,
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
        funding_txid: Option<Txid>,
    ) -> Result<()> {
        match db::get_payment(*payment_hash)? {
            Some(_) => {
                db::update_payment(
                    *payment_hash,
                    htlc_status,
                    amt_msat,
                    fee_msat,
                    preimage,
                    secret,
                    funding_txid,
                )?;
            }
            None => {
                db::insert_payment(
                    *payment_hash,
                    PaymentInfo {
                        preimage,
                        secret,
                        status: htlc_status,
                        amt_msat,
                        fee_msat,
                        flow,
                        timestamp: OffsetDateTime::now_utc(),
                        description: "".to_string(),
                        invoice: None,
                        funding_txid,
                    },
                )?;
            }
        }

        Ok(())
    }
    fn get_payment(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<Option<(PaymentHash, PaymentInfo)>> {
        db::get_payment(*payment_hash)
    }
    fn all_payments(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        db::get_payments()
    }

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

    // Channels

    fn upsert_channel(&self, channel: Channel) -> Result<()> {
        db::upsert_channel(channel)
    }

    fn get_channel(&self, user_channel_id: &str) -> Result<Option<Channel>> {
        db::get_channel(user_channel_id)
    }

    fn all_non_pending_channels(&self) -> Result<Vec<Channel>> {
        db::get_all_non_pending_channels()
    }

    fn get_announced_channel(&self, counterparty_pubkey: PublicKey) -> Result<Option<Channel>> {
        db::get_announced_channel(counterparty_pubkey)
    }

    fn get_channel_by_payment_hash(&self, payment_hash: String) -> Result<Option<Channel>> {
        db::get_channel_by_payment_hash(payment_hash)
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
