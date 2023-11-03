use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::channel::UserChannelId;
use ln_dlc_node::lightning;
use ln_dlc_node::lightning::events::Event;
use ln_dlc_node::ln::common_handlers;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::ChannelManager;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::Storage;
use ln_dlc_node::EventHandlerTrait;
use ln_dlc_node::EventSender;
use std::sync::Arc;
use tokio::task::block_in_place;
use uuid::Uuid;

pub struct EventHandler<S> {
    pub(crate) node: Arc<Node<S>>,
}

impl<S> EventHandler<S>
where
    S: Storage + Send + Sync + 'static,
{
    pub fn new(node: Arc<Node<S>>) -> Self {
        Self { node }
    }
}

#[async_trait]
impl<S> EventHandlerTrait for EventHandler<S>
where
    S: Storage + Send + Sync + 'static,
{
    fn event_sender(&self) -> &Option<EventSender> {
        &None
    }

    async fn match_event(&self, event: Event) -> Result<()> {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                user_channel_id,
            } => {
                common_handlers::handle_funding_generation_ready(
                    &self.node,
                    user_channel_id,
                    counterparty_node_id,
                    output_script,
                    channel_value_satoshis,
                    temporary_channel_id,
                )
                .await?;
            }
            Event::PaymentClaimed {
                payment_hash,
                purpose,
                amount_msat,
                receiver_node_id: _,
            } => {
                common_handlers::handle_payment_claimed(
                    &self.node,
                    amount_msat,
                    payment_hash,
                    purpose,
                );
            }
            Event::PaymentSent {
                payment_preimage,
                payment_hash,
                fee_paid_msat,
                ..
            } => {
                common_handlers::handle_payment_sent(
                    &self.node,
                    payment_hash,
                    payment_preimage,
                    fee_paid_msat,
                )?;
            }
            Event::OpenChannelRequest {
                counterparty_node_id,
                funding_satoshis,
                push_msat,
                temporary_channel_id,
                ..
            } => {
                handle_open_channel_request_0_conf(
                    &self.node.channel_manager,
                    counterparty_node_id,
                    funding_satoshis,
                    push_msat,
                    temporary_channel_id,
                )?;
            }
            Event::PaymentPathSuccessful {
                payment_id,
                payment_hash,
                path,
            } => {
                tracing::info!(?payment_id, ?payment_hash, ?path, "Payment path successful");
            }
            Event::PaymentPathFailed { payment_hash, .. } => {
                tracing::warn!(
                    payment_hash = %payment_hash.0.to_hex(),
                "Payment path failed");
            }
            Event::PaymentFailed { payment_hash, .. } => {
                common_handlers::handle_payment_failed(&self.node, payment_hash);
            }
            Event::PaymentForwarded {
                prev_channel_id,
                next_channel_id,
                fee_earned_msat,
                claim_from_onchain_tx,
                outbound_amount_forwarded_msat,
            } => {
                common_handlers::handle_payment_forwarded(
                    &self.node,
                    prev_channel_id,
                    next_channel_id,
                    claim_from_onchain_tx,
                    fee_earned_msat,
                    outbound_amount_forwarded_msat,
                );
            }
            Event::PendingHTLCsForwardable { time_forwardable } => {
                common_handlers::handle_pending_htlcs_forwardable(
                    self.node.channel_manager.clone(),
                    time_forwardable,
                );
            }
            Event::SpendableOutputs { outputs } => {
                common_handlers::handle_spendable_outputs(&self.node, outputs)?;
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id,
            } => {
                self.handle_channel_closed(user_channel_id, reason, channel_id)?;
            }
            Event::DiscardFunding {
                channel_id,
                transaction,
            } => {
                common_handlers::handle_discard_funding(transaction, channel_id);
            }
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::ChannelReady {
                channel_id,
                counterparty_node_id,
                user_channel_id,
                ..
            } => {
                self.handle_channel_ready(user_channel_id, channel_id, counterparty_node_id)?;
            }
            Event::HTLCHandlingFailed {
                prev_channel_id,
                failed_next_destination,
            } => {
                common_handlers::handle_htlc_handling_failed(
                    prev_channel_id,
                    failed_next_destination,
                );
            }
            Event::PaymentClaimable {
                receiver_node_id: _,
                payment_hash,
                onion_fields: _,
                amount_msat,
                counterparty_skimmed_fee_msat: _,
                purpose,
                via_channel_id: _,
                via_user_channel_id: _,
                claim_deadline: _,
            } => {
                common_handlers::handle_payment_claimable(
                    &self.node.channel_manager,
                    payment_hash,
                    purpose,
                    amount_msat,
                )?;
            }
            Event::HTLCIntercepted { .. } => {
                tracing::error!(
                    ?event,
                    "The maker should not support interceptable invoices!"
                );
            }
            Event::ChannelPending {
                channel_id,
                user_channel_id: _,
                former_temporary_channel_id,
                counterparty_node_id,
                funding_txo,
            } => {
                let former_temporary_channel_id =
                    former_temporary_channel_id.unwrap_or([0; 32]).to_hex();
                tracing::debug!(
                    channel_id = channel_id.to_hex(),
                    former_temporary_channel_id,
                    counterparty_node_id = counterparty_node_id.to_string(),
                    funding_txo_tx_id = funding_txo.txid.to_string(),
                    funding_txo_tx_vout = funding_txo.vout,
                    "Channel pending"
                )
            }
            Event::BumpTransaction(_) => {
                tracing::error!("We do not support anchor outputs yet");
            }
        };

        Ok(())
    }
}

impl<S> EventHandler<S>
where
    S: Storage,
{
    pub fn handle_channel_ready(
        &self,
        user_channel_id: u128,
        channel_id: [u8; 32],
        counterparty_node_id: PublicKey,
    ) -> Result<()>
    where
        S: Storage,
    {
        block_in_place(|| {
            let user_channel_id = UserChannelId::from(user_channel_id).to_string();

            tracing::info!(
                user_channel_id,
                channel_id = %channel_id.to_hex(),
                counterparty = %counterparty_node_id.to_string(),
                "Channel ready"
            );

            let channel_details = self
                .node
                .channel_manager
                .get_channel_details(&channel_id)
                .ok_or(anyhow!(
                    "Failed to get channel details by channel_id {}",
                    channel_id.to_hex()
                ))?;

            let channel = self.node.storage.get_channel(&user_channel_id)?;
            let channel = Channel::open_channel(channel, channel_details)?;
            self.node.storage.upsert_channel(channel)?;

            Ok(())
        })
    }

    pub fn handle_channel_closed(
        &self,
        user_channel_id: u128,
        reason: lightning::events::ClosureReason,
        channel_id: [u8; 32],
    ) -> Result<(), anyhow::Error> {
        block_in_place(|| {
            let user_channel_id = Uuid::from_u128(user_channel_id).to_string();
            tracing::info!(
                %user_channel_id,
                channel_id = %channel_id.to_hex(),
                ?reason,
                "Channel closed",
            );

            if let Some(channel) = self.node.storage.get_channel(&user_channel_id)? {
                let channel = Channel::close_channel(channel, reason.clone());
                self.node.storage.upsert_channel(channel)?;
            }

            self.node
                .sub_channel_manager
                .notify_ln_channel_closed(channel_id, &reason)?;

            anyhow::Ok(())
        })?;
        Ok(())
    }
}

fn handle_open_channel_request_0_conf(
    channel_manager: &Arc<ChannelManager>,
    counterparty_node_id: PublicKey,
    funding_satoshis: u64,
    push_msat: u64,
    temporary_channel_id: [u8; 32],
) -> Result<()> {
    let counterparty = counterparty_node_id.to_string();
    tracing::info!(
        counterparty,
        funding_satoshis,
        push_msat,
        "Accepting open channel request"
    );
    channel_manager
        .accept_inbound_channel_from_trusted_peer_0conf(
            &temporary_channel_id,
            &counterparty_node_id,
            0,
        )
        .map_err(|e| anyhow!("{e:?}"))
        .context("To be able to accept a 0-conf channel")?;
    Ok(())
}
