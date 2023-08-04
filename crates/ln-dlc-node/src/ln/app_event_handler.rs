use super::event_handler::handlers;
use super::event_handler::EventSender;
use super::event_handler::PendingInterceptedHtlcs;
use crate::node::Node;
use crate::node::Storage;
use crate::EventHandlerTrait;
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::hashes::hex::ToHex;
use lightning::util::events::Event;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Event handler for the mobile 10101 app.
// TODO: Move it out of this crate
pub struct AppEventHandler<S> {
    pub(crate) node: Arc<Node<S>>,
    pub(crate) pending_intercepted_htlcs: PendingInterceptedHtlcs,
    pub(crate) event_sender: Option<EventSender>,
}

impl<S> AppEventHandler<S>
where
    S: Storage + Send + Sync + 'static,
{
    pub fn new(node: Arc<Node<S>>, event_sender: Option<EventSender>) -> Self {
        Self {
            node,
            event_sender,
            pending_intercepted_htlcs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl<S> EventHandlerTrait for AppEventHandler<S>
where
    S: Storage + Send + Sync + 'static,
{
    fn event_sender(&self) -> &Option<EventSender> {
        &self.event_sender
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
                handlers::handle_funding_generation_ready(
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
                handlers::handle_payment_claimed(&self.node, amount_msat, payment_hash, purpose);
            }
            Event::PaymentSent {
                payment_preimage,
                payment_hash,
                fee_paid_msat,
                ..
            } => {
                handlers::handle_payment_sent(
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
                // TODO: only accept 0-conf from the coordinator.
                // right now we are using the same conf for app and coordinator, meaning this will
                // be called for both. We however do not want to accept 0-conf channels from someone
                // outside of our domain.
                handlers::handle_open_channel_request_trusted_0_conf(
                    &self.node.channel_manager,
                    counterparty_node_id,
                    funding_satoshis,
                    push_msat,
                    temporary_channel_id,
                )?;
                tracing::info!(
                    "Ignoring open channel request from {} for {} satoshis. App does not support opening chanels",
                    counterparty_node_id,
                    funding_satoshis
                );
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
                handlers::handle_payment_failed(&self.node, payment_hash);
            }
            Event::PaymentForwarded {
                prev_channel_id,
                next_channel_id,
                fee_earned_msat,
                claim_from_onchain_tx,
            } => {
                handlers::handle_payment_forwarded(
                    &self.node,
                    prev_channel_id,
                    next_channel_id,
                    claim_from_onchain_tx,
                    fee_earned_msat,
                );
            }
            Event::PendingHTLCsForwardable { time_forwardable } => {
                handlers::handle_pending_htlcs_forwardable(
                    self.node.channel_manager.clone(),
                    time_forwardable,
                );
            }
            Event::SpendableOutputs { outputs } => {
                handlers::handle_spendable_outputs(&self.node, outputs)?;
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id,
            } => {
                handlers::handle_channel_closed(
                    &self.node,
                    &self.pending_intercepted_htlcs,
                    user_channel_id,
                    reason,
                    channel_id,
                )?;
            }
            Event::DiscardFunding {
                channel_id,
                transaction,
            } => {
                handlers::handle_discard_funding(transaction, channel_id);
            }
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::ChannelReady {
                channel_id,
                counterparty_node_id,
                user_channel_id,
                ..
            } => {
                handlers::handle_channel_ready(
                    &self.node,
                    &self.pending_intercepted_htlcs,
                    user_channel_id,
                    channel_id,
                    counterparty_node_id,
                )?;
            }
            Event::HTLCHandlingFailed {
                prev_channel_id,
                failed_next_destination,
            } => {
                handlers::handle_htlc_handling_failed(prev_channel_id, failed_next_destination);
            }
            Event::PaymentClaimable {
                receiver_node_id: _,
                payment_hash,
                amount_msat,
                purpose,
                via_channel_id: _,
                via_user_channel_id: _,
            } => {
                handlers::handle_payment_claimable(
                    &self.node.channel_manager,
                    payment_hash,
                    purpose,
                    amount_msat,
                )?;
            }
            Event::HTLCIntercepted {
                intercept_id,
                requested_next_hop_scid,
                payment_hash,
                inbound_amount_msat,
                expected_outbound_amount_msat,
            } => {
                handlers::handle_intercepted_htlc(
                    &self.node,
                    &self.pending_intercepted_htlcs,
                    intercept_id,
                    payment_hash,
                    requested_next_hop_scid,
                    inbound_amount_msat,
                    expected_outbound_amount_msat,
                )
                .await?;
            }
        };

        Ok(())
    }
}
