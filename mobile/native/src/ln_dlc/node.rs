use crate::db;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::trade::order;
use crate::trade::position;
use crate::trade::position::PositionState;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::TransactionDetails;
use dlc_messages::sub_channel::SubChannelCloseFinalize;
use dlc_messages::sub_channel::SubChannelRevoke;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use lightning::chain::keysinterface::DelayedPaymentOutputDescriptor;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::chain::keysinterface::StaticPaymentOutputDescriptor;
use lightning::chain::transaction::OutPoint;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning_invoice::Invoice;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::PaymentDetails;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::transaction::Transaction;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::MillisatAmount;
use ln_dlc_node::PaymentFlow;
use ln_dlc_node::PaymentInfo;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node<NodeStorage>>,
    _running: Arc<RunningNode>,
    /// The order-matching fee invoice to be paid once the current trade is executed.
    ///
    /// We assume that only one trade can be executed at a time.
    pub order_matching_fee_invoice: Arc<RwLock<Option<Invoice>>>,
}

impl Node {
    pub fn new(node: Arc<node::Node<NodeStorage>>, running: RunningNode) -> Self {
        Self {
            inner: node,
            _running: Arc::new(running),
            order_matching_fee_invoice: Arc::new(RwLock::new(None)),
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
            lightning: value.off_chain,
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

    pub fn get_wallet_balances(&self) -> Result<Balances> {
        let on_chain = self.inner.get_on_chain_balance()?.confirmed;
        let off_chain = self.inner.get_ldk_balance().available();

        Ok(Balances {
            on_chain,
            off_chain,
        })
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
                    "Failed to process DLC message: {e:#}"
                );
            }
        }
    }

    fn process_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %dlc_message_name(&msg),
            "Processing message"
        );

        let resp = match &msg {
            Message::OnChain(_) | Message::Channel(_) => self
                .inner
                .dlc_manager
                .on_dlc_message(&msg, node_id)
                .with_context(|| {
                    format!(
                        "Failed to handle {} message from {node_id}",
                        dlc_message_name(&msg)
                    )
                })?,
            Message::SubChannel(ref msg) => {
                let resp = self
                    .inner
                    .sub_channel_manager
                    .on_sub_channel_message(msg, &node_id)
                    .with_context(|| {
                        format!(
                            "Failed to handle {} message from {node_id}",
                            sub_channel_message_name(msg)
                        )
                    })?
                    .map(Message::SubChannel);

                // Some incoming messages require extra action from our part for the protocol to
                // continue
                match msg {
                    SubChannelMessage::Offer(offer) => {
                        let channel_id = offer.channel_id;

                        // TODO: We should probably verify that: (1) the counterparty is the
                        // coordinator and (2) the DLC channel offer is expected and correct.
                        self.inner
                            .accept_dlc_channel_offer(&channel_id)
                            .with_context(|| {
                                format!(
                                    "Failed to accept DLC channel offer for channel {}",
                                    hex::encode(channel_id)
                                )
                            })?
                    }
                    SubChannelMessage::CloseOffer(offer) => {
                        let channel_id = offer.channel_id;

                        // TODO: We should probably verify that: (1) the counterparty is the
                        // coordinator and (2) the DLC channel close offer is expected and correct.
                        self.inner
                            .accept_dlc_channel_collaborative_settlement(&channel_id)
                            .with_context(|| {
                                format!(
                                    "Failed to accept DLC channel close offer for channel {}",
                                    hex::encode(channel_id)
                                )
                            })?;
                    }
                    _ => (),
                };

                resp
            }
        };

        // TODO(holzeis): It would be nice if dlc messages are also propagated via events, so the
        // receiver can decide what events to process and we can skip this component specific logic
        // here.
        if let Message::Channel(channel_message) = &msg {
            match channel_message {
                ChannelMessage::RenewOffer(r) => {
                    tracing::info!("Automatically accepting a rollover position");
                    let (accept_renew_offer, counterparty_pubkey) =
                        self.inner.dlc_manager.accept_renew_offer(&r.channel_id)?;

                    self.send_dlc_message(
                        counterparty_pubkey,
                        Message::Channel(ChannelMessage::RenewAccept(accept_renew_offer)),
                    )?;

                    let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
                        r.contract_info.get_closest_maturity_date() as i64,
                    )?;
                    position::handler::rollover_position(expiry_timestamp)?;
                }
                ChannelMessage::RenewRevoke(_) => {
                    tracing::info!("Finished rollover position");
                    // After handling the `RenewRevoke` message, we need to do some post-processing
                    // based on the fact that the DLC channel has been updated.
                    position::handler::set_position_state(PositionState::Open)?;

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::Rollover(TaskStatus::Success),
                    ));
                }
                // ignoring all other channel events.
                _ => (),
            }
        }

        // After handling the `Revoke` message, we need to do some post-processing based on the fact
        // that the DLC channel has been established
        if let Message::SubChannel(SubChannelMessage::Revoke(SubChannelRevoke {
            channel_id, ..
        })) = msg
        {
            let (accept_collateral, expiry_timestamp) = self
                .inner
                .get_collateral_and_expiry_for_confirmed_contract(channel_id)?;

            let filled_order = order::handler::order_filled()
                .context("Cannot mark order as filled for confirmed DLC")?;

            position::handler::update_position_after_dlc_creation(
                filled_order,
                accept_collateral,
                expiry_timestamp,
            )
            .context("Failed to update position after DLC creation")?;

            // Sending always a recover dlc background notification success message here as we do
            // not know if we might have reached this state after a restart. This event is only
            // received by the UI at the moment indicating that the dialog can be closed.
            // If the dialog is not open, this event would be simply ignored by the UI.
            //
            // FIXME(holzeis): We should not require that event and align the UI handling with
            // waiting for an order execution in the happy case with waiting for an
            // order execution after an in between restart. For now it was the easiest
            // to go parallel to that implementation so that we don't have to touch it.
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::RecoverDlc(TaskStatus::Success),
            ));

            if let Err(e) = self.pay_order_matching_fee(&channel_id) {
                tracing::error!("{e:#}");
            }
        }

        if let Some(msg) = resp {
            self.send_dlc_message(node_id, msg)?;
        }

        Ok(())
    }

    pub fn send_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            to = %node_id,
            kind = %dlc_message_name(&msg),
            "Sending message"
        );

        self.inner
            .dlc_message_handler
            .send_message(node_id, msg.clone());

        // After sending the `CloseFinalize` message, we need to do some post-processing based on
        // the fact that the DLC channel has been closed
        if let Message::SubChannel(SubChannelMessage::CloseFinalize(SubChannelCloseFinalize {
            channel_id,
            ..
        })) = msg
        {
            let filled_order = order::handler::order_filled()?;
            position::handler::update_position_after_dlc_closure(Some(filled_order))
                .context("Failed to update position after DLC closure")?;

            // Sending always a recover dlc background notification success message here as we do
            // not know if we might have reached this state after a restart. This event is only
            // received by the UI at the moment indicating that the dialog can be closed.
            // If the dialog is not open, this event would be simply ignored by the UI.
            //
            // FIXME(holzeis): We should not require that event and align the UI handling with
            // waiting for an order execution in the happy case with waiting for an
            // order execution after an in between restart. For now it was the easiest
            // to go parallel to that implementation so that we don't have to touch it.
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::RecoverDlc(TaskStatus::Success),
            ));

            if let Err(e) = self.pay_order_matching_fee(&channel_id) {
                tracing::error!("{e:#}");
            }
        };

        Ok(())
    }

    fn pay_order_matching_fee(&self, channel_id: &[u8; 32]) -> Result<()> {
        let channel_id_str = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_str, "Paying order-matching fee");

        let invoice = self
            .order_matching_fee_invoice
            .write()
            .take()
            .context("Expected to have to pay an order-matching fee invoice")?;

        // Without this `sleep` the coordinator can end up not generating the corresponding
        // `PaymentClaimable` event, leaving the payment in a pending state seemingly forever
        std::thread::sleep(Duration::from_secs(5));

        self.inner
            .send_payment(&invoice)
            .context("Failed order-matching fee invoice payment")?;

        tracing::info!(
            channel_id = %channel_id_str,
            payment_hash = %invoice.payment_hash(),
            description = ?invoice.description(),
            amount_msat = ?invoice.amount_milli_satoshis(),
            "Triggered payment of order-matching fee"
        );

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
