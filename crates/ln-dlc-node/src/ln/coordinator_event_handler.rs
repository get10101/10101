use super::common_handlers;
use super::event_handler::EventSender;
use super::event_handler::PendingInterceptedHtlcs;
use crate::channel::FakeScid;
use crate::config::HTLC_INTERCEPTED_CONNECTION_TIMEOUT;
use crate::ln::common_handlers::fail_intercepted_htlc;
use crate::ln::event_handler::InterceptionDetails;
use crate::node::ChannelManager;
use crate::node::Node;
use crate::node::Storage;
use crate::EventHandlerTrait;
use crate::CONFIRMATION_TARGET;
use crate::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT_MAX;
use crate::LIQUIDITY_MULTIPLIER;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::ln::channelmanager::InterceptId;
use lightning::ln::PaymentHash;
use lightning::util::events::Event;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Event handler for the coordinator node.
// TODO: Move it out of this crate
pub struct CoordinatorEventHandler<S> {
    pub(crate) node: Arc<Node<S>>,
    pub(crate) pending_intercepted_htlcs: PendingInterceptedHtlcs,
    pub(crate) event_sender: Option<EventSender>,
}

impl<S> CoordinatorEventHandler<S>
where
    S: Storage + Sync + Send + 'static,
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
impl<S> EventHandlerTrait for CoordinatorEventHandler<S>
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
                temporary_channel_id,
                counterparty_node_id,
                funding_satoshis,
                push_msat,
                ..
            } => {
                handle_open_channel_request(
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
            } => {
                common_handlers::handle_payment_forwarded(
                    &self.node,
                    prev_channel_id,
                    next_channel_id,
                    claim_from_onchain_tx,
                    fee_earned_msat,
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
                common_handlers::handle_channel_closed(
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
                common_handlers::handle_channel_ready(
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
                common_handlers::handle_htlc_handling_failed(
                    prev_channel_id,
                    failed_next_destination,
                );
            }
            Event::PaymentClaimable {
                receiver_node_id: _,
                payment_hash,
                amount_msat,
                purpose,
                via_channel_id: _,
                via_user_channel_id: _,
            } => {
                common_handlers::handle_payment_claimable(
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
                handle_intercepted_htlc(
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

fn handle_open_channel_request(
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
    let user_channel_id = 0;
    channel_manager
        .accept_inbound_channel(
            &temporary_channel_id,
            &counterparty_node_id,
            user_channel_id,
        )
        .map_err(|e| anyhow!("{e:?}"))
        .context("To be able to accept a 0-conf channel")?;
    Ok(())
}

/// Handle an [`Event::HTLCIntercepted`].
pub(crate) async fn handle_intercepted_htlc<S>(
    node: &Arc<Node<S>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    intercept_id: InterceptId,
    payment_hash: PaymentHash,
    requested_next_hop_scid: u64,
    inbound_amount_msat: u64,
    expected_outbound_amount_msat: u64,
) -> Result<()>
where
    S: Storage,
{
    let res = handle_intercepted_htlc_internal(
        node,
        pending_intercepted_htlcs,
        intercept_id,
        payment_hash,
        requested_next_hop_scid,
        inbound_amount_msat,
        expected_outbound_amount_msat,
    )
    .await;

    if let Err(ref e) = res {
        tracing::error!("Failed to handle HTLCIntercepted event: {e:#}");
        fail_intercepted_htlc(&node.channel_manager, &intercept_id);
    }

    res
}

pub(crate) async fn handle_intercepted_htlc_internal<S>(
    node: &Arc<Node<S>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    intercept_id: InterceptId,
    payment_hash: PaymentHash,
    requested_next_hop_scid: u64,
    inbound_amount_msat: u64,
    expected_outbound_amount_msat: u64,
) -> Result<()>
where
    S: Storage,
{
    let intercept_id_str = intercept_id.0.to_hex();
    let payment_hash = payment_hash.0.to_hex();

    tracing::info!(
        intercept_id = %intercept_id_str,
        requested_next_hop_scid,
        payment_hash,
        inbound_amount_msat,
        expected_outbound_amount_msat,
        "Intercepted HTLC"
    );

    let target_node_id = {
        node.fake_channel_payments
            .lock()
            .get(&requested_next_hop_scid)
            .copied()
    }
    .with_context(|| {
        format!(
            "Could not forward the intercepted HTLC because we didn't have a node registered \
                 with fake scid {requested_next_hop_scid}"
        )
    })?;

    tokio::time::timeout(HTLC_INTERCEPTED_CONNECTION_TIMEOUT, async {
        loop {
            if node
                .peer_manager
                .get_peer_node_ids()
                .iter()
                .any(|(id, _)| *id == target_node_id)
            {
                tracing::info!(
                    %target_node_id,
                    %payment_hash,
                    "Found connection with target of intercepted HTLC"
                );

                return;
            }

            tracing::debug!(
                %target_node_id,
                %payment_hash,
                "Waiting for connection with target of intercepted HTLC"
            );
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
    .await
    .context("Timed out waiting to get connection with target of interceptable HTLC")?;

    // We only support one channel between coordinator and app. Also, we are unfortunately using
    // interceptable HTLCs for regular payments (not just to open JIT channels). With all this
    // in mind, if the coordinator (the only party that can handle this event) has a channel
    // with the target of this payment we must treat this interceptable HTLC as a regular
    // payment.
    if let Some(channel) = node
        .channel_manager
        .list_channels()
        .iter()
        .find(|channel_details| channel_details.counterparty.node_id == target_node_id)
    {
        node.channel_manager
            .forward_intercepted_htlc(
                intercept_id,
                &channel.channel_id,
                channel.counterparty.node_id,
                expected_outbound_amount_msat,
            )
            .map_err(|e| anyhow!("Failed to forward intercepted HTLC: {e:?}"))?;

        return Ok(());
    }

    let opt_max_allowed_fee = node
        .wallet
        .inner()
        .settings()
        .await
        .max_allowed_tx_fee_rate_when_opening_channel;
    if let Some(max_allowed_tx_fee) = opt_max_allowed_fee {
        let current_fee = node
            .fee_rate_estimator
            .get_est_sat_per_1000_weight(CONFIRMATION_TARGET);

        ensure!(
            max_allowed_tx_fee >= current_fee,
            "Not opening JIT channel because the fee is too high"
        );
    }

    let channel_value = expected_outbound_amount_msat / 1000 * LIQUIDITY_MULTIPLIER;
    ensure!(
        channel_value <= JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT_MAX,
        "Failed to open channel because maximum channel value exceeded"
    );

    let fake_scid = FakeScid::new(requested_next_hop_scid);
    let mut shadow_channel = node
        .storage
        .get_channel_by_fake_scid(fake_scid)
        .with_context(|| format!("Failed to load channel by fake SCID {fake_scid}"))?
        .with_context(|| format!("Could not find shadow channel for fake SCID {fake_scid}"))?;

    shadow_channel.outbound = channel_value;

    node.storage
        .upsert_channel(shadow_channel.clone())
        .with_context(|| format!("Failed to upsert shadow channel: {shadow_channel}"))?;

    let mut ldk_config = *node.ldk_config.read();
    ldk_config.channel_handshake_config.announced_channel = false;

    let temp_channel_id = node
        .channel_manager
        .create_channel(
            target_node_id,
            channel_value,
            0,
            shadow_channel.user_channel_id.to_u128(),
            Some(ldk_config),
        )
        .map_err(|e| anyhow!("Failed to open JIT channel: {e:?}"))?;

    tracing::info!(
        peer = %target_node_id,
        %payment_hash,
        temp_channel_id = %temp_channel_id.to_hex(),
        "Started JIT channel creation for intercepted HTLC"
    );

    pending_intercepted_htlcs.lock().insert(
        target_node_id,
        InterceptionDetails {
            id: intercept_id,
            expected_outbound_amount_msat,
        },
    );

    Ok(())
}
