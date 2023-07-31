use crate::db;
use crate::trade::order;
use crate::trade::position;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::TransactionDetails;
use dlc_messages::sub_channel::SubChannelCloseFinalize;
use dlc_messages::sub_channel::SubChannelRevoke;
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
use ln_dlc_node::node::rust_dlc_manager::contract::signed_contract::SignedContract;
use ln_dlc_node::node::rust_dlc_manager::contract::Contract;
use ln_dlc_node::node::rust_dlc_manager::Storage as _;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::PaymentDetails;
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
    /// The order-matching fee invoice to be paid once the current trade is executed.
    ///
    /// We assume that only one trade can be executed at a time.
    pub order_matching_fee_invoice: Arc<RwLock<Option<Invoice>>>,
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
    pub fn get_seed_phrase(&self) -> Vec<String> {
        self.inner.get_seed_phrase()
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
                    "Failed to process message: {e:#}"
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

        let resp = match msg {
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

        // After handling the `Revoke` message, we need to do some post-processing based on the fact
        // that the DLC channel has been established
        if let Message::SubChannel(SubChannelMessage::Revoke(SubChannelRevoke {
            channel_id, ..
        })) = msg
        {
            let contracts = self.inner.dlc_manager.get_store().get_contracts()?;

            let accept_collateral = contracts
                .iter()
                // Taking the first `Confirmed` contract we find is just a
                // heuristic. Ideally we would be able to match against the
                // `ContractId` or the `ChannelId`, but the information is not
                // guaranteed to be there
                .find_map(|contract| match contract {
                    Contract::Confirmed(SignedContract {
                        accepted_contract, ..
                    }) => Some(accepted_contract.accept_params.collateral),
                    _ => None,
                })
                .with_context(|| {
                    format!(
                        "Confirmed contract not found for channel ID: {}",
                        hex::encode(channel_id)
                    )
                })?;

            let filled_order = order::handler::order_filled()
                .context("Cannot mark order as filled for confirmed DLC")?;

            if let Err(e) = self.pay_order_matching_fee(&channel_id) {
                tracing::error!("{e:#}");
            }

            position::handler::update_position_after_dlc_creation(filled_order, accept_collateral)
                .context("Failed to update position after DLC creation")?
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
            match order::handler::order_filled() {
                Ok(filled_order) => {
                    if let Err(e) = self.pay_order_matching_fee(&channel_id) {
                        tracing::error!("{e:#}");
                    }

                    position::handler::update_position_after_dlc_closure(filled_order)
                        .context("Failed to update position after DLC closure")?;
                }
                // TODO: Should we charge for the order-matching fee if there is no order????
                Err(e) => {
                    tracing::warn!("Could not find a filling position in the database. Maybe because the coordinator closed an expired position. Error: {e:#}");

                    tokio::spawn(async {
                        match position::handler::close_position().await {
                            Ok(_) => tracing::info!("Successfully closed expired position."),
                            Err(e) => tracing::error!("Critical Error! We have a DLC but were unable to set the order to filled. Error: {e:?}")
                        }
                    });
                }
            };
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
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
    ) -> Result<()> {
        match db::get_payment(*payment_hash)? {
            Some(_) => {
                db::update_payment(*payment_hash, htlc_status, amt_msat, preimage, secret)?;
            }
            None => {
                db::insert_payment(
                    *payment_hash,
                    PaymentInfo {
                        preimage,
                        secret,
                        status: htlc_status,
                        amt_msat,
                        flow,
                        timestamp: OffsetDateTime::now_utc(),
                        description: "".to_string(),
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
