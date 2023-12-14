use super::common_handlers;
use super::event_handler::EventSender;
use super::event_handler::PendingInterceptedHtlcs;
use crate::channel::Channel;
use crate::channel::ChannelState;
use crate::channel::UserChannelId;
use crate::config::HTLC_INTERCEPTED_CONNECTION_TIMEOUT;
use crate::ln::common_handlers::fail_intercepted_htlc;
use crate::ln::event_handler::InterceptionDetails;
use crate::node::ChannelManager;
use crate::node::LiquidityRequest;
use crate::node::Node;
use crate::node::Storage;
use crate::storage::TenTenOneStorage;
use crate::EventHandlerTrait;
use crate::CONFIRMATION_TARGET;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::subchannel::LNChannelManager;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::events::Event;
use lightning::ln::channelmanager::InterceptId;
use lightning::ln::ChannelId;
use lightning::ln::PaymentHash;
use parking_lot::Mutex;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::block_in_place;

/// Event handler for the coordinator node.
// TODO: Move it out of this crate
pub struct CoordinatorEventHandler<S: TenTenOneStorage, N: Storage> {
    pub(crate) node: Arc<Node<S, N>>,
    pub(crate) pending_intercepted_htlcs: PendingInterceptedHtlcs,
    pub(crate) event_sender: Option<EventSender>,
}

impl<S: TenTenOneStorage, N: Storage> CoordinatorEventHandler<S, N> {
    pub fn new(node: Arc<Node<S, N>>, event_sender: Option<EventSender>) -> Self {
        Self {
            node,
            event_sender,
            pending_intercepted_htlcs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl<S: TenTenOneStorage + 'static, N: Storage + Send + Sync + 'static> EventHandlerTrait
    for CoordinatorEventHandler<S, N>
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
                htlcs: _,
                sender_intended_total_msat: _,
            } => {
                common_handlers::handle_payment_claimed(
                    &self.node,
                    amount_msat,
                    None,
                    None,
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
            Event::SpendableOutputs {
                outputs,
                channel_id: _,
            } => {
                // TODO(holzeis): Update shadow channel to store the commitment transaction closing
                // the channel.
                common_handlers::handle_spendable_outputs(&self.node, outputs)?;
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id,
                counterparty_node_id: _,
                channel_capacity_sats: _,
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
            Event::ProbeSuccessful {
                payment_id, path, ..
            } => common_handlers::handle_probe_successful(&self.node, payment_id, path).await,
            Event::ProbeFailed { payment_id, .. } => {
                common_handlers::handle_probe_failed(&self.node, payment_id).await
            }
            Event::ChannelReady {
                channel_id,
                counterparty_node_id,
                user_channel_id,
                ..
            } => {
                block_in_place(|| {
                    let res = handle_channel_ready_internal(
                        &self.node,
                        &self.pending_intercepted_htlcs,
                        user_channel_id,
                        channel_id,
                        counterparty_node_id,
                    );

                    if let Err(ref e) = res {
                        tracing::error!("Failed to handle ChannelReady event: {e:#}");

                        // If the `ChannelReady` event was associated with a pending intercepted
                        // HTLC, we must fail it to unlock the funds of all
                        // the nodes along the payment route
                        if let Some(interception) = self
                            .pending_intercepted_htlcs
                            .lock()
                            .get(&counterparty_node_id)
                        {
                            fail_intercepted_htlc(&self.node.channel_manager, &interception.id);
                        }
                    }

                    res
                })?;
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
            Event::ChannelPending {
                channel_id,
                user_channel_id: _,
                former_temporary_channel_id,
                counterparty_node_id,
                funding_txo,
            } => {
                let former_temporary_channel_id = former_temporary_channel_id
                    .unwrap_or(ChannelId([0; 32]))
                    .to_hex();
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

fn handle_channel_ready_internal<S: TenTenOneStorage, N: Storage>(
    node: &Arc<Node<S, N>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    user_channel_id: u128,
    channel_id: ChannelId,
    counterparty_node_id: PublicKey,
) -> Result<()> {
    let user_channel_id = UserChannelId::from(user_channel_id).to_string();

    tracing::info!(
        user_channel_id,
        channel_id = %channel_id.to_hex(),
        counterparty = %counterparty_node_id.to_string(),
        "Channel ready"
    );

    let channel_details = node
        .channel_manager
        .get_channel_details(&channel_id)
        .ok_or(anyhow!(
            "Failed to get channel details by channel_id {}",
            channel_id.to_hex()
        ))?;

    let channel = node.node_storage.get_channel(&user_channel_id)?;
    let channel = Channel::open_channel(channel, channel_details)?;
    node.node_storage.upsert_channel(channel.clone())?;

    if let Some(interception) = pending_intercepted_htlcs.lock().get(&counterparty_node_id) {
        tracing::info!(
            intercept_id = %interception.id.0.to_hex(),
            counterparty = %counterparty_node_id.to_string(),
            forward_amount_msat = %interception.expected_outbound_amount_msat,
            "Pending intercepted HTLC found, forwarding payment"
        );

        let fee_msat = channel.fee_sats.map(|fee| fee * 1000).unwrap_or(0);
        node.channel_manager
            .forward_intercepted_htlc(
                interception.id,
                &channel_id,
                counterparty_node_id,
                interception.expected_outbound_amount_msat - fee_msat,
            )
            .map_err(|e| anyhow!("{e:?}"))
            .context("Failed to forward intercepted HTLC")?;
    }

    Ok(())
}

fn handle_open_channel_request<S: TenTenOneStorage, N: Storage>(
    channel_manager: &Arc<ChannelManager<S, N>>,
    counterparty_node_id: PublicKey,
    funding_satoshis: u64,
    push_msat: u64,
    temporary_channel_id: ChannelId,
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

#[allow(clippy::too_many_arguments)]
/// Handle an [`Event::HTLCIntercepted`].
pub(crate) async fn handle_intercepted_htlc<S: TenTenOneStorage, N: Storage>(
    node: &Arc<Node<S, N>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    intercept_id: InterceptId,
    payment_hash: PaymentHash,
    requested_next_hop_scid: u64,
    inbound_amount_msat: u64,
    expected_outbound_amount_msat: u64,
) -> Result<()> {
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_intercepted_htlc_internal<S: TenTenOneStorage, N: Storage>(
    node: &Arc<Node<S, N>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    intercept_id: InterceptId,
    payment_hash: PaymentHash,
    requested_next_hop_scid: u64,
    inbound_amount_msat: u64,
    expected_outbound_amount_msat: u64,
) -> Result<()> {
    let intercept_id_str = intercept_id.0.to_hex();
    let payment_hash = payment_hash.0.to_hex();

    let liquidity_request = {
        node.fake_channel_payments
            .lock()
            .get(&requested_next_hop_scid)
            .cloned()
    }
    .with_context(|| {
        format!(
            "Could not forward the intercepted HTLC because we didn't have a node registered \
             with fake scid {requested_next_hop_scid}"
        )
    })?;

    tracing::info!(
        intercept_id = %intercept_id_str,
        requested_next_hop_scid,
        payment_hash,
        inbound_amount_msat,
        expected_outbound_amount_msat,
        ?liquidity_request,
        "Intercepted HTLC"
    );

    let peer_id = liquidity_request.trader_id;

    // TODO(holzeis): Send push notification if the user is receiving an intercepted payment but not
    // online. This may improve the onboarding success rate.
    tokio::time::timeout(HTLC_INTERCEPTED_CONNECTION_TIMEOUT, async {
        loop {
            if node
                .peer_manager
                .get_peer_node_ids()
                .iter()
                .any(|(id, _)| *id == peer_id)
            {
                tracing::info!(
                    %peer_id,
                    %payment_hash,
                    "Found connection with target of intercepted HTLC"
                );

                return;
            }

            tracing::debug!(
                %peer_id,
                %payment_hash,
                "Waiting for connection with target of intercepted HTLC"
            );
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
    .await
    .context("Timed out waiting to get connection with target of interceptable HTLC")?;

    if let Some(channel) = node
        .channel_manager
        .list_channels()
        .iter()
        .find(|channel_details| channel_details.counterparty.node_id == peer_id)
    {
        tracing::warn!(trader_id=%channel.counterparty.node_id, channel_id=channel.channel_id.to_hex(),
            "Intercepted a payment to a channel that already exist. That should not happen!");

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

    let max_counterparty_fund_amount_msat = liquidity_request.max_deposit_sats * 1000;
    ensure!(
        expected_outbound_amount_msat <= max_counterparty_fund_amount_msat,
        "Failed to open channel because maximum fund amount exceeded, \
         expected_outbound_amount_msat: {expected_outbound_amount_msat} > \
         max_counterparty_fund_amount_msat: {max_counterparty_fund_amount_msat}"
    );

    let opt_max_allowed_fee = node
        .wallet
        .ldk_wallet()
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

    let channel_value_sats =
        calculate_channel_value(expected_outbound_amount_msat, &liquidity_request);

    let user_channel_id = liquidity_request.user_channel_id;
    let mut shadow_channel = node
        .node_storage
        .get_channel(&user_channel_id.to_string())
        .with_context(|| format!("Failed to load channel by user_channel_id {user_channel_id}"))?
        .with_context(|| {
            format!("Could not find shadow channel for user channel id {user_channel_id}")
        })?;

    shadow_channel.outbound_sats = channel_value_sats;
    shadow_channel.channel_state = ChannelState::Pending;
    shadow_channel.fee_sats = Some(liquidity_request.fee_sats);

    node.node_storage
        .upsert_channel(shadow_channel.clone())
        .with_context(|| format!("Failed to upsert shadow channel: {shadow_channel}"))?;

    let mut ldk_config = *node.ldk_config.read();
    ldk_config.channel_handshake_config.announced_channel = false;

    let temp_channel_id = node
        .channel_manager
        .create_channel(
            peer_id,
            channel_value_sats,
            0,
            shadow_channel.user_channel_id.to_u128(),
            Some(ldk_config),
        )
        .map_err(|e| anyhow!("Failed to open JIT channel: {e:?}"))?;

    tracing::info!(
        %peer_id,
        %payment_hash,
        channel_value_sats,
        temp_channel_id = %temp_channel_id.to_hex(),
        "Started JIT channel creation for intercepted HTLC"
    );

    pending_intercepted_htlcs.lock().insert(
        peer_id,
        InterceptionDetails {
            id: intercept_id,
            expected_outbound_amount_msat,
        },
    );

    Ok(())
}

/// Calculates the channel value in sats based on the inital amount received by the user and the
/// liquidity request.
pub fn calculate_channel_value(
    expected_outbound_amount_msat: u64,
    liquidity_request: &LiquidityRequest,
) -> u64 {
    let expected_outbound_amount =
        Decimal::from(expected_outbound_amount_msat) / Decimal::from(1000);
    let trade_up_to_sats = Decimal::from(liquidity_request.trade_up_to_sats);
    let coordinator_leverage =
        Decimal::try_from(liquidity_request.coordinator_leverage).expect("to fit into decimal");

    let channel_value = expected_outbound_amount + (trade_up_to_sats / coordinator_leverage);

    channel_value.to_u64().expect("to fit into u64")
}

#[cfg(test)]
mod tests {
    use crate::ln::coordinator_event_handler::calculate_channel_value;
    use crate::node::LiquidityRequest;
    use bitcoin::secp256k1::PublicKey;
    use std::str::FromStr;

    #[test]
    fn test_calculate_channel_value() {
        let dummy_pub_key = PublicKey::from_str(
            "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
        )
        .expect("valid pubkey");

        let capacity = 200_000;
        for i in 1..5 {
            let request = LiquidityRequest {
                user_channel_id: Default::default(),
                liquidity_option_id: 1,
                trader_id: dummy_pub_key,
                trade_up_to_sats: capacity * i,
                max_deposit_sats: capacity * i,
                coordinator_leverage: i as f32,
                fee_sats: 5_000,
            };

            let channel_value_sat = calculate_channel_value(10_000_000, &request);

            assert_eq!(210_000, channel_value_sat)
        }
    }
}
