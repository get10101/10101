use crate::dlc_custom_signer::CustomKeysManager;
use crate::ln::coordinator_config;
use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::invoice::HTLCStatus;
use crate::node::ChannelManager;
use crate::util;
use crate::FakeChannelPaymentRequests;
use crate::MillisatAmount;
use crate::NetworkGraph;
use crate::PaymentInfo;
use crate::PaymentInfoStorage;
use crate::PendingInterceptedHtlcs;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::Secp256k1;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::routing::gossip::NodeId;
use lightning::util::events::Event;
use lightning::util::events::PaymentPurpose;
use rand::thread_rng;
use rand::Rng;
use std::cmp;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime;

pub struct EventHandler {
    runtime_handle: runtime::Handle,
    channel_manager: Arc<ChannelManager>,
    wallet: Arc<LnDlcWallet>,
    network_graph: Arc<NetworkGraph>,
    keys_manager: Arc<CustomKeysManager>,
    inbound_payments: PaymentInfoStorage,
    outbound_payments: PaymentInfoStorage,
    fake_channel_payments: FakeChannelPaymentRequests,
    pending_intercepted_htlcs: PendingInterceptedHtlcs,
}

#[allow(clippy::too_many_arguments)]
impl EventHandler {
    pub(crate) fn new(
        runtime_handle: runtime::Handle,
        channel_manager: Arc<ChannelManager>,
        wallet: Arc<LnDlcWallet>,
        network_graph: Arc<NetworkGraph>,
        keys_manager: Arc<CustomKeysManager>,
        inbound_payments: PaymentInfoStorage,
        outbound_payments: PaymentInfoStorage,
        fake_channel_payments: FakeChannelPaymentRequests,
        pending_intercepted_htlcs: PendingInterceptedHtlcs,
    ) -> Self {
        Self {
            runtime_handle,
            channel_manager,
            wallet,
            network_graph,
            keys_manager,
            inbound_payments,
            outbound_payments,
            fake_channel_payments,
            pending_intercepted_htlcs,
        }
    }

    fn match_event(&self, event: Event) -> Result<()> {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                ..
            } => {
                let target_blocks = 2;

                // Have wallet put the inputs into the transaction such that the output
                // is satisfied and then sign the funding transaction
                let funding_tx_result = self.wallet.inner().construct_funding_transaction(
                    &output_script,
                    channel_value_satoshis,
                    target_blocks,
                );

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
                let mut payments = self.inbound_payments.lock().unwrap();
                match payments.entry(payment_hash) {
                    Entry::Occupied(mut e) => {
                        let payment = e.get_mut();
                        payment.status = HTLCStatus::Succeeded;
                        payment.preimage = payment_preimage;
                        payment.secret = payment_secret;
                    }
                    Entry::Vacant(e) => {
                        e.insert(PaymentInfo {
                            preimage: payment_preimage,
                            secret: payment_secret,
                            status: HTLCStatus::Succeeded,
                            amt_msat: MillisatAmount(Some(amount_msat)),
                            timestamp: OffsetDateTime::now_utc(),
                        });
                    }
                }
            }
            Event::PaymentSent {
                payment_preimage,
                payment_hash,
                fee_paid_msat,
                ..
            } => {
                let mut payments = self.outbound_payments.lock().unwrap();
                let amount_msat = match payments.entry(payment_hash) {
                    Entry::Occupied(mut e) => {
                        let payment = e.get_mut();
                        payment.preimage = Some(payment_preimage);
                        payment.status = HTLCStatus::Succeeded;

                        payment.amt_msat
                    }
                    Entry::Vacant(e) => {
                        tracing::warn!(
                            "Got PaymentSent event without matching outbound payment on record"
                        );

                        let amt_msat = MillisatAmount(None);
                        e.insert(PaymentInfo {
                            preimage: Some(payment_preimage),
                            secret: None,
                            status: HTLCStatus::Succeeded,
                            amt_msat,
                            timestamp: OffsetDateTime::now_utc(),
                        });

                        amt_msat
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
                    "Accepting 0-conf channel request"
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
            Event::PaymentPathSuccessful { .. } => {}
            Event::PaymentPathFailed { .. } => {}
            Event::PaymentFailed { payment_hash, .. } => {
                tracing::warn!(
                    payment_hash = %hex::encode(payment_hash.0),
                    "Failed to send payment to payment hash: exhausted payment retry attempts",
                );

                let mut payments = self.outbound_payments.lock().unwrap();
                match payments.entry(payment_hash) {
                    Entry::Occupied(mut e) => {
                        let payment = e.get_mut();
                        payment.status = HTLCStatus::Failed;
                    }
                    Entry::Vacant(e) => {
                        e.insert(PaymentInfo {
                            preimage: None,
                            secret: None,
                            status: HTLCStatus::Failed,
                            amt_msat: MillisatAmount(None),
                            timestamp: OffsetDateTime::now_utc(),
                        });
                    }
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
                let forwarding_channel_manager = self.channel_manager.clone();
                let min = time_forwardable.as_millis() as u64;
                tokio::spawn(async move {
                    let millis_to_sleep = thread_rng().gen_range(min..(min * 5));
                    tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
                    forwarding_channel_manager.process_pending_htlc_forwards();
                });
            }
            Event::SpendableOutputs { outputs } => {
                let destination_address = self.wallet.inner().get_unused_address()?;
                let output_descriptors = &outputs.iter().collect::<Vec<_>>();
                let tx_feerate = self
                    .wallet
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
                let spending_tx = self.keys_manager.spend_spendable_outputs(
                    output_descriptors,
                    Vec::new(),
                    destination_address.script_pubkey(),
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
            Event::DiscardFunding { .. } => {
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

                let pending_intercepted_htlcs = self.pending_intercepted_htlcs.lock().unwrap();

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
            Event::HTLCHandlingFailed { .. } => {}
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
                    let fake_channel_payments = self.fake_channel_payments.lock().unwrap();
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

                // FIXME: This will set the channel capacity to twice the amount that is
                // transferred or the `JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT` ensuring there
                // is enough liquidity in the channel. This is arbitrary and needs
                // to be computed, but good enough for our test cases to support any
                // amount to be sent.
                let channel_value = cmp::max(
                    JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT,
                    expected_outbound_amount_msat * 2 / 1000,
                );

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

                let pending_intercepted_htlcs = self.pending_intercepted_htlcs.clone();
                let mut pending_intercepted_htlcs = pending_intercepted_htlcs.lock().unwrap();
                pending_intercepted_htlcs.insert(
                    target_node_id,
                    (intercept_id, expected_outbound_amount_msat),
                );
            }
        };

        Ok(())
    }
}

impl lightning::util::events::EventHandler for EventHandler {
    fn handle_event(&self, event: Event) {
        tracing::info!(?event, "Received event");

        self.runtime_handle.block_on(async {
            let event_str = format!("{event:?}");

            match self.match_event(event) {
                Ok(()) => tracing::debug!(event = ?event_str, "Successfully handled event"),
                Err(e) => tracing::error!("Failed to handle event. Error {e:#}"),
            }
        })
    }
}
