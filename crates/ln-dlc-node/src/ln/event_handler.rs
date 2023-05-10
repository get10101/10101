use crate::dlc_custom_signer::CustomKeysManager;
use crate::ln::coordinator_config;
use crate::ln::HTLC_INTERCEPTED_CONNECTION_TIMEOUT;
use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::invoice::HTLCStatus;
use crate::node::ChannelManager;
use crate::node::PaymentPersister;
use crate::util;
use crate::FakeChannelPaymentRequests;
use crate::MillisatAmount;
use crate::NetworkGraph;
use crate::PaymentFlow;
use crate::PaymentInfo;
use crate::PeerManager;
use crate::PendingInterceptedHtlcs;
use crate::RequestedScid;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::ln::channelmanager::InterceptId;
use lightning::routing::gossip::NodeId;
use lightning::util::events::Event;
use lightning::util::events::PaymentPurpose;
use rand::thread_rng;
use rand::Rng;
use secp256k1_zkp::Secp256k1;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::MutexGuard;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime;

///  The speed at which we want a transaction to confirm used for feerate estimation.
///
/// We set it to high priority because the channel funding transaction should be included fast.
const CONFIRMATION_TARGET: ConfirmationTarget = ConfirmationTarget::HighPriority;

pub struct EventHandler<P> {
    runtime_handle: runtime::Handle,
    channel_manager: Arc<ChannelManager>,
    wallet: Arc<LnDlcWallet>,
    network_graph: Arc<NetworkGraph>,
    keys_manager: Arc<CustomKeysManager>,
    payment_persister: Arc<P>,
    fake_channel_payments: FakeChannelPaymentRequests,
    pending_intercepted_htlcs: PendingInterceptedHtlcs,
    peer_manager: Arc<PeerManager>,
}

#[allow(clippy::too_many_arguments)]
impl<P> EventHandler<P>
where
    P: PaymentPersister,
{
    pub(crate) fn new(
        runtime_handle: runtime::Handle,
        channel_manager: Arc<ChannelManager>,
        wallet: Arc<LnDlcWallet>,
        network_graph: Arc<NetworkGraph>,
        keys_manager: Arc<CustomKeysManager>,
        payment_persister: Arc<P>,
        fake_channel_payments: FakeChannelPaymentRequests,
        pending_intercepted_htlcs: PendingInterceptedHtlcs,
        peer_manager: Arc<PeerManager>,
    ) -> Self {
        Self {
            runtime_handle,
            channel_manager,
            wallet,
            network_graph,
            keys_manager,
            payment_persister,
            fake_channel_payments,
            pending_intercepted_htlcs,
            peer_manager,
        }
    }

    async fn match_event(&self, event: Event) -> Result<()> {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                ..
            } => {
                tracing::info!(
                    %counterparty_node_id,
                    "Funding generation ready for channel with counterparty {}",
                    counterparty_node_id
                );

                let target_blocks = CONFIRMATION_TARGET;

                // Have wallet put the inputs into the transaction such that the output
                // is satisfied and then sign the funding transaction
                let funding_tx_result = self
                    .wallet
                    .inner()
                    .create_funding_transaction(
                        output_script,
                        channel_value_satoshis,
                        target_blocks,
                    )
                    .await;

                let funding_tx = match funding_tx_result {
                    Ok(funding_tx) => funding_tx,
                    Err(err) => {
                        tracing::error!(
                            %err,
                            "Cannot open channel due to not being able to create funding tx"
                        );
                        self.channel_manager
                            .close_channel(&temporary_channel_id, &counterparty_node_id)
                            .map_err(|e| anyhow!("{e:?}"))?;

                        return Ok(());
                    }
                };

                // Give the funding transaction back to LDK for opening the channel.

                if let Err(err) = self.channel_manager.funding_transaction_generated(
                    &temporary_channel_id,
                    &counterparty_node_id,
                    funding_tx,
                ) {
                    tracing::error!(?err, "Channel went away before we could fund it. The peer disconnected or refused the channel");
                }
            }
            Event::PaymentClaimed {
                payment_hash,
                purpose,
                amount_msat,
                receiver_node_id: _,
            } => {
                tracing::info!(
                    %amount_msat,
                    payment_hash = %hex::encode(payment_hash.0),
                    "Claimed payment",
                );

                let (payment_preimage, payment_secret) = match purpose {
                    PaymentPurpose::InvoicePayment {
                        payment_preimage,
                        payment_secret,
                        ..
                    } => (payment_preimage, Some(payment_secret)),
                    PaymentPurpose::SpontaneousPayment(preimage) => (Some(preimage), None),
                };

                let amount_msat = MillisatAmount(Some(amount_msat));
                if let Err(e) = self.payment_persister.merge(
                    &payment_hash,
                    PaymentFlow::Inbound,
                    amount_msat,
                    HTLCStatus::Succeeded,
                    payment_preimage,
                    payment_secret,
                ) {
                    tracing::error!(payment_hash = %hex::encode(payment_hash.0), "Failed to update claimed payment: {e:#}")
                }
            }
            Event::PaymentSent {
                payment_preimage,
                payment_hash,
                fee_paid_msat,
                ..
            } => {
                let amount_msat = match self.payment_persister.get(&payment_hash) {
                    Ok(Some((_, PaymentInfo { amt_msat, .. }))) => {
                        let amount_msat = MillisatAmount(None);
                        if let Err(e) = self.payment_persister.merge(
                            &payment_hash,
                            PaymentFlow::Outbound,
                            amount_msat,
                            HTLCStatus::Succeeded,
                            Some(payment_preimage),
                            None,
                        ) {
                            tracing::error!(payment_hash = %hex::encode(payment_hash.0), "Failed to update sent payment: {e:#}");

                            return Ok(());
                        }

                        amt_msat
                    }
                    Ok(None) => {
                        tracing::warn!(
                            "Got PaymentSent event without matching outbound payment on record"
                        );

                        let amt_msat = MillisatAmount(None);
                        if let Err(e) = self.payment_persister.insert(
                            payment_hash,
                            PaymentInfo {
                                preimage: Some(payment_preimage),
                                secret: None,
                                status: HTLCStatus::Succeeded,
                                amt_msat,
                                flow: PaymentFlow::Outbound,
                                timestamp: OffsetDateTime::now_utc(),
                            },
                        ) {
                            tracing::error!(
                                payment_hash = %hex::encode(payment_hash.0),
                                "Failed to insert sent payment: {e:#}"
                            );
                        }

                        amt_msat
                    }
                    Err(e) => {
                        tracing::error!(
                            payment_hash = %hex::encode(payment_hash.0),
                            "Failed to verify payment state before handling sent payment: {e:#}"
                        );

                        return Ok(());
                    }
                };

                tracing::info!(
                    amount_msat = ?amount_msat.0,
                    fee_paid_msat = ?fee_paid_msat,
                    payment_hash = %hex::encode(payment_hash.0),
                    preimage_hash = %hex::encode(payment_preimage.0),
                    "Successfully sent payment",
                );
            }
            Event::OpenChannelRequest {
                temporary_channel_id,
                counterparty_node_id,
                funding_satoshis,
                push_msat,
                ..
            } => {
                // TODO: only accept 0-conf from the coordinator.
                // right now we are using the same conf for app and coordinator, meaning this will
                // be called for both. We however do not want to accept 0-conf channels from someone
                // outside of our domain.
                let counterparty = counterparty_node_id.to_string();
                tracing::info!(
                    counterparty,
                    funding_satoshis,
                    push_msat,
                    "Accepting open channel request"
                );
                self.channel_manager
                    .accept_inbound_channel_from_trusted_peer_0conf(
                        &temporary_channel_id,
                        &counterparty_node_id,
                        0,
                    )
                    .map_err(|e| anyhow!("{e:?}"))
                    .context("To be able to accept a 0-conf channel")?;
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
                payment_hash = %hex::encode(payment_hash.0),
                "Payment path failed");
            }
            Event::PaymentFailed { payment_hash, .. } => {
                tracing::warn!(
                    payment_hash = %hex::encode(payment_hash.0),
                    "Failed to send payment to payment hash: exhausted payment retry attempts",
                );

                let amount_msat = MillisatAmount(None);
                if let Err(e) = self.payment_persister.merge(
                    &payment_hash,
                    PaymentFlow::Outbound,
                    amount_msat,
                    HTLCStatus::Failed,
                    None,
                    None,
                ) {
                    tracing::error!(payment_hash = %hex::encode(payment_hash.0), "Failed to update failed payment: {e:#}")
                }
            }
            Event::PaymentForwarded {
                prev_channel_id,
                next_channel_id,
                fee_earned_msat,
                claim_from_onchain_tx,
            } => {
                let read_only_network_graph = self.network_graph.read_only();
                let nodes = read_only_network_graph.nodes();
                let channels = self.channel_manager.list_channels();

                let node_str = |channel_id: &Option<[u8; 32]>| match channel_id {
                    None => String::new(),
                    Some(channel_id) => match channels.iter().find(|c| c.channel_id == *channel_id)
                    {
                        None => String::new(),
                        Some(channel) => {
                            match nodes.get(&NodeId::from_pubkey(&channel.counterparty.node_id)) {
                                None => " from private node".to_string(),
                                Some(node) => match &node.announcement_info {
                                    None => " from unnamed node".to_string(),
                                    Some(announcement) => {
                                        format!("node {}", announcement.alias)
                                    }
                                },
                            }
                        }
                    },
                };
                let channel_str = |channel_id: &Option<[u8; 32]>| {
                    channel_id
                        .map(|channel_id| format!(" with channel {}", hex::encode(channel_id)))
                        .unwrap_or_default()
                };
                let from_prev_str = format!(
                    "{}{}",
                    node_str(&prev_channel_id),
                    channel_str(&prev_channel_id)
                );
                let to_next_str = format!(
                    "{}{}",
                    node_str(&next_channel_id),
                    channel_str(&next_channel_id)
                );

                let from_onchain_str = if claim_from_onchain_tx {
                    "from onchain downstream claim"
                } else {
                    "from HTLC fulfill message"
                };
                if let Some(fee_earned) = fee_earned_msat {
                    tracing::info!(
                        "Forwarded payment{}{}, earning {} msat {}",
                        from_prev_str,
                        to_next_str,
                        fee_earned,
                        from_onchain_str
                    );
                } else {
                    tracing::info!(
                        "Forwarded payment{}{}, claiming onchain {}",
                        from_prev_str,
                        to_next_str,
                        from_onchain_str
                    );
                }
            }
            Event::PendingHTLCsForwardable { time_forwardable } => {
                tracing::debug!(
                    time_forwardable = ?time_forwardable,
                    "Pending HTLCs are forwardable"
                );
                let forwarding_channel_manager = self.channel_manager.clone();
                let min = time_forwardable.as_millis() as u64;
                tokio::spawn(async move {
                    let millis_to_sleep = thread_rng().gen_range(min..(min * 5));
                    tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
                    forwarding_channel_manager.process_pending_htlc_forwards();
                });
            }
            Event::SpendableOutputs { outputs } => {
                let ldk_outputs = outputs
                    .iter()
                    .filter(|output| {
                        // `StaticOutput`s are sent to the node's on-chain wallet directly
                        !matches!(output, SpendableOutputDescriptor::StaticOutput { .. })
                    })
                    .collect::<Vec<_>>();

                let destination_script = self.wallet.inner().get_last_unused_address()?;
                let tx_feerate = self
                    .wallet
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
                let spending_tx = self.keys_manager.spend_spendable_outputs(
                    &ldk_outputs,
                    vec![],
                    destination_script.script_pubkey(),
                    tx_feerate,
                    &Secp256k1::new(),
                )?;
                self.wallet.broadcast_transaction(&spending_tx);
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id: _,
            } => {
                let channel = hex::encode(channel_id);
                tracing::info!(
                    %channel,
                    ?reason,
                    "\nChannel closed",
                );
            }
            Event::DiscardFunding {
                channel_id,
                transaction,
            } => {
                let tx_hex = serialize_hex(&transaction);
                tracing::info!(
                    channel_id = %hex::encode(channel_id),
                    %tx_hex,
                    "Discarding funding transaction"
                );
                // A "real" node should probably "lock" the UTXOs spent in funding transactions
                // until the funding transaction either confirms, or this event is
                // generated.
            }
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::ChannelReady {
                channel_id,
                counterparty_node_id,
                ..
            } => {
                tracing::info!(
                    channel_id = %hex::encode(channel_id),
                    counterparty = %counterparty_node_id.to_string(),
                    "Channel ready"
                );

                let pending_intercepted_htlcs = self.pending_intercepted_htlcs_lock();

                if let Some((intercept_id, expected_outbound_amount_msat)) =
                    pending_intercepted_htlcs.get(&counterparty_node_id)
                {
                    tracing::info!(
                        intercept_id = %hex::encode(intercept_id.0),
                        counterparty = %counterparty_node_id.to_string(),
                        forward_amount_msat = %expected_outbound_amount_msat,
                        "Pending intercepted HTLC found, forwarding payment"
                    );

                    self.channel_manager
                        .forward_intercepted_htlc(
                            *intercept_id,
                            &channel_id,
                            counterparty_node_id,
                            *expected_outbound_amount_msat,
                        )
                        .map_err(|e| anyhow!("{e:?}"))
                        .context("Failed to forward intercepted HTLC")?;
                }
            }
            Event::HTLCHandlingFailed {
                prev_channel_id,
                failed_next_destination,
            } => {
                tracing::info!(
                    prev_channel_id = %hex::encode(prev_channel_id),
                    failed_next_destination = ?failed_next_destination,
                    "HTLC handling failed"
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
                let payment_hash = util::hex_str(&payment_hash.0);

                let preimage = match purpose {
                    PaymentPurpose::InvoicePayment {
                        payment_preimage: Some(preimage),
                        ..
                    }
                    | PaymentPurpose::SpontaneousPayment(preimage) => preimage,
                    _ => {
                        tracing::debug!("Received PaymentClaimable event without preimage");
                        return Ok(());
                    }
                };

                tracing::info!(%payment_hash, %amount_msat, "Received payment");

                self.channel_manager.claim_funds(preimage);
            }
            Event::HTLCIntercepted {
                intercept_id,
                requested_next_hop_scid,
                payment_hash,
                inbound_amount_msat,
                expected_outbound_amount_msat,
            } => {
                let intercepted_id = hex::encode(intercept_id.0);
                let payment_hash = hex::encode(payment_hash.0);
                tracing::info!(
                    intercepted_id,
                    requested_next_hop_scid,
                    payment_hash,
                    inbound_amount_msat,
                    expected_outbound_amount_msat,
                    "Intercepted HTLC"
                );

                let target_node_id = {
                    let fake_channel_payments = self.fake_channel_payments_lock();
                    match fake_channel_payments.get(&requested_next_hop_scid) {
                        None => {
                            tracing::warn!(fake_scid = requested_next_hop_scid, "Could not forward the intercepted HTLC because we didn't have a node registered with said fake scid");

                            if let Err(err) =
                                self.channel_manager.fail_intercepted_htlc(intercept_id)
                            {
                                tracing::error!("Could not fail intercepted htlc {err:?}")
                            }

                            return Ok(());
                        }
                        Some(target_node_id) => *target_node_id,
                    }
                };

                // FIXME: This is only a temporary quick fix for the MVP and should be fixed
                // properly. Ideally the app would run in the background. Not necessarily for ever
                // but for at least a couple of seconds / minutes
                tokio::time::timeout(Duration::from_secs(HTLC_INTERCEPTED_CONNECTION_TIMEOUT), async {
                    loop {
                        if self.peer_manager
                            .get_peer_node_ids()
                            .iter()
                            .any(|(id, _)| *id == target_node_id) {
                            tracing::info!(%target_node_id, "Found connection to target peer. Continuing HTLCIntercepted event.");

                            return;
                        }
                        tracing::debug!(%target_node_id, "Waiting for target node to come online.");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }).await?;

                // if we have already a channel with them, we try to forward the payment.
                if let Some(channel) = self
                    .channel_manager
                    .list_channels()
                    .iter()
                    // The coordinator can only have one channel with each app. Hence, if we find a
                    // channel with the target of the intercepted HTLC, we know
                    // that it is the only channel between coordinator and
                    // target app and we can forward the intercepted HTLC through it.
                    .find(|channel_details| channel_details.counterparty.node_id == target_node_id)
                {
                    // Note, the forward intercepted htlc might fail due to insufficient balance,
                    // since we do not check yet if the channel outbound capacity is sufficient.
                    if let Err(error) = self.channel_manager.forward_intercepted_htlc(
                        intercept_id,
                        &channel.channel_id,
                        channel.counterparty.node_id,
                        expected_outbound_amount_msat,
                    ) {
                        tracing::warn!(?error, "Failed to forward intercepted HTLC");

                        self.channel_manager
                            .fail_intercepted_htlc(intercept_id)
                            .map_err(|e| anyhow!("{e:?}"))?;
                    }

                    return Ok(());
                }

                let opt_max_allowed_fee = self
                    .wallet
                    .inner()
                    .settings()
                    .await
                    .max_allowed_tx_fee_rate_when_opening_channel;

                // Do not open a channel if the fee is too high for us
                if let Some(max_allowed_tx_fee) = opt_max_allowed_fee {
                    let current_fee = self
                        .wallet
                        .inner()
                        .get_est_sat_per_1000_weight(CONFIRMATION_TARGET);
                    if max_allowed_tx_fee < current_fee {
                        tracing::warn!(%max_allowed_tx_fee, %current_fee, "Not opening a channel because the fee is too high");
                        if let Err(err) = self.channel_manager.fail_intercepted_htlc(intercept_id) {
                            tracing::error!("Could not fail intercepted htlc {err:?}")
                        }
                        return Ok(());
                    }
                }

                // Currently the channel capacity is fixed for the beta program
                let channel_value = JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;

                let mut user_config = coordinator_config();
                // We are overwriting the coordinators channel handshake configuration to prevent
                // the just-in-time-channel from being announced (private). This is required as both
                // parties need to agree on this configuration. For other channels, like with the
                // channel to an external node we want this channel to be announced (public).
                // NOTE: we want private channels with the mobile app, as this will allow us to make
                // use of 0-conf channels.
                user_config.channel_handshake_config.announced_channel = false;

                // NOTE: We actually might want to override the `UserConfig`
                // for this just-in-time channel so that the
                // intercepted HTLC is allowed to be added to the
                // channel according to its
                // `max_inbound_htlc_value_in_flight_percent_of_channel`
                // configuration value
                let temp_channel_id = match self.channel_manager.create_channel(
                    target_node_id,
                    channel_value,
                    0,
                    0,
                    Some(user_config),
                ) {
                    Ok(temp_channel_id) => temp_channel_id,
                    Err(err) => {
                        tracing::warn!(?err, "Failed to open just in time channel");

                        if let Err(err) = self
                            .channel_manager
                            .fail_intercepted_htlc(intercept_id)
                            .map_err(|e| anyhow!("{e:?}"))
                        {
                            tracing::error!("Could not fail intercepted htlc {err:?}");
                        };

                        return Ok(());
                    }
                };

                tracing::info!(
                    peer = %target_node_id,
                    temp_channel_id = %hex::encode(temp_channel_id),
                    "Started channel creation for in-flight payment"
                );

                let mut pending_intercepted_htlcs = self.pending_intercepted_htlcs_lock();
                pending_intercepted_htlcs.insert(
                    target_node_id,
                    (intercept_id, expected_outbound_amount_msat),
                );
            }
        };

        Ok(())
    }
}

impl<P> lightning::util::events::EventHandler for EventHandler<P>
where
    P: PaymentPersister,
{
    fn handle_event(&self, event: Event) {
        tracing::info!(?event, "Received event");

        self.runtime_handle.block_on(async {
            let event_str = format!("{event:?}");

            match self.match_event(event).await {
                Ok(()) => tracing::debug!(event = ?event_str, "Successfully handled event"),
                Err(e) => tracing::error!("Failed to handle event. Error {e:#}"),
            }
        })
    }
}

impl<P> EventHandler<P> {
    fn fake_channel_payments_lock(&self) -> MutexGuard<HashMap<RequestedScid, PublicKey>> {
        self.fake_channel_payments
            .lock()
            .expect("Mutex to not be poisoned")
    }

    fn pending_intercepted_htlcs_lock(&self) -> MutexGuard<HashMap<PublicKey, (InterceptId, u64)>> {
        self.pending_intercepted_htlcs
            .lock()
            .expect("Mutex to not be poisoned")
    }
}
