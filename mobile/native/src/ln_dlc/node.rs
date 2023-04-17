use crate::db;
use crate::trade::order;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::TransactionDetails;
use dlc_messages::sub_channel::SubChannelFinalize;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::rust_dlc_manager::contract::signed_contract::SignedContract;
use ln_dlc_node::node::rust_dlc_manager::contract::Contract;
use ln_dlc_node::node::rust_dlc_manager::Storage;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::PaymentDetails;
use ln_dlc_node::node::PaymentPersister;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::MillisatAmount;
use ln_dlc_node::PaymentFlow;
use ln_dlc_node::PaymentInfo;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node<Payments>>,
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
        let off_chain = self.inner.get_ldk_balance().available;

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

    pub fn process_incoming_messages(&self) -> Result<()> {
        let messages = self
            .inner
            .dlc_message_handler
            .get_and_clear_received_messages();

        for (node_id, msg) in messages {
            let msg_name = dlc_message_name(&msg);
            if let Err(e) = self.process_incoming_message(node_id, msg) {
                tracing::error!(
                    from = %node_id,
                    msg = %msg_name,
                    "Failed to process incoming message: {e:#}"
                );
                continue;
            }
        }

        // NOTE: According to the docs of `process_events` we shouldn't have to call this since we
        // use `lightning-net-tokio`. But we copied this from `p2pderivatives/ldk-sample`
        if self.inner.dlc_message_handler.has_pending_messages() {
            self.inner.peer_manager.process_events();
        }

        Ok(())
    }

    fn process_incoming_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        match msg {
            Message::OnChain(_) | Message::Channel(_) => {
                tracing::debug!(from = %node_id, "Processing DLC-manager message");
                let resp = self
                    .inner
                    .dlc_manager
                    .on_dlc_message(&msg, node_id)
                    .map_err(|e| anyhow!(e.to_string()))?;

                if let Some(msg) = resp {
                    tracing::debug!(to = %node_id, "Sending DLC-manager message");
                    self.inner.dlc_message_handler.send_message(node_id, msg);
                }
            }
            Message::SubChannel(incoming_msg) => {
                tracing::debug!(
                    from = %node_id,
                    msg = %sub_channel_message_name(&incoming_msg),
                    "Processing DLC channel message"
                );
                let reply_msg = self
                    .inner
                    .sub_channel_manager
                    .on_sub_channel_message(&incoming_msg, &node_id)
                    .map_err(|e| anyhow!(e.to_string()))?;

                if let SubChannelMessage::Offer(offer) = &incoming_msg {
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

                if let SubChannelMessage::CloseOffer(offer) = &incoming_msg {
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

                if let Some(reply_msg) = reply_msg {
                    tracing::debug!(
                        to = %node_id,
                        msg = %sub_channel_message_name(&reply_msg),
                        "Sending DLC channel message"
                    );
                    self.inner
                        .dlc_message_handler
                        .send_message(node_id, Message::SubChannel(reply_msg.clone()));

                    match reply_msg {
                        SubChannelMessage::Finalize(SubChannelFinalize { channel_id, .. }) => {
                            let contract = self
                                .inner
                                .dlc_manager
                                .get_store()
                                .get_contract(&channel_id)
                                .map_err(|e| anyhow!("{e:#}"))?
                                .with_context(|| {
                                    format!(
                                        "Contract not found for channel ID: {}",
                                        hex::encode(channel_id)
                                    )
                                })?;

                            let accept_collateral = match contract {
                                Contract::Confirmed(SignedContract {
                                    accepted_contract, ..
                                }) => accepted_contract.accept_params.collateral,
                                state => {
                                    bail!("Expected contract in confirmed state, got {state:?}");
                                }
                            };

                            let filled_order = order::handler::order_filled()
                                .context("Cannot mark order as filled for confirmed DLC")?;

                            position::handler::update_position_after_dlc_creation(
                                filled_order,
                                accept_collateral,
                            )
                            .context("Failed to update position after DLC creation")?
                        }
                        SubChannelMessage::CloseFinalize(_) => {
                            match order::handler::order_filled() {
                                Ok(filled_order) => {
                                    position::handler::update_position_after_dlc_closure(
                                        filled_order,
                                    )
                                    .context("Failed to update position after DLC closure")?;
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Could not find a filling position in the database.
                                         Maybe because the coordinator closed an expired position.
                                         Error: {e:#}"
                                    );

                                    tokio::spawn(async {
                                        match position::handler::close_position().await {
                                                Ok(_) => tracing::info!("Successfully closed expired position."),
                                                Err(e) => tracing::error!("Critical Error! We have a DLC but were unable to set the order to filled. Error: {e:?}")
                                            }
                                    });
                                }
                            };
                        }
                        _ => (),
                    }
                }
            }
        }

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
pub struct Payments;

impl PaymentPersister for Payments {
    fn insert(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()> {
        db::insert_payment(payment_hash, info)
    }
    fn merge(
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
                    },
                )?;
            }
        }

        Ok(())
    }
    fn get(&self, payment_hash: &PaymentHash) -> Result<Option<(PaymentHash, PaymentInfo)>> {
        db::get_payment(*payment_hash)
    }
    fn all(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        db::get_payments()
    }
}
