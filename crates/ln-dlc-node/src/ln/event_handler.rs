use crate::ln_dlc_wallet::LnDlcWallet;
use crate::util;
use crate::ChannelManager;
use crate::HTLCStatus;
use crate::MillisatAmount;
use crate::NetworkGraph;
use crate::PaymentInfo;
use crate::PaymentInfoStorage;
use bitcoin::secp256k1::Secp256k1;
use bitcoin_bech32::WitnessProgram;
use dlc_manager::custom_signer::CustomKeysManager;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::routing::gossip::NodeId;
use lightning::util::events::Event;
use lightning::util::events::PaymentPurpose;
use rand::thread_rng;
use rand::Rng;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime;

pub struct EventHandler {
    runtime_handle: runtime::Handle,
    channel_manager: Arc<ChannelManager>,
    wallet: Arc<LnDlcWallet>,
    network_graph: Arc<NetworkGraph>,
    keys_manager: Arc<CustomKeysManager>,
    inbound_payments: PaymentInfoStorage,
    outbound_payments: PaymentInfoStorage,
}

impl EventHandler {
    pub(crate) fn new(
        runtime_handle: runtime::Handle,
        channel_manager: Arc<ChannelManager>,
        wallet: Arc<LnDlcWallet>,
        network_graph: Arc<NetworkGraph>,
        keys_manager: Arc<CustomKeysManager>,
        inbound_payments: PaymentInfoStorage,
        outbound_payments: PaymentInfoStorage,
    ) -> Self {
        Self {
            runtime_handle,
            channel_manager,
            wallet,
            network_graph,
            keys_manager,
            inbound_payments,
            outbound_payments,
        }
    }
}

impl lightning::util::events::EventHandler for EventHandler {
    fn handle_event(&self, event: Event) {
        tracing::info!(?event, "Received event");

        self.runtime_handle.block_on(async {
            match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                ..
            } => {
                // Construct the raw transaction with one output, that is paid the amount of the
                // channel.
                let _addr = WitnessProgram::from_scriptpubkey(
                    &output_script[..],
                    bitcoin_bech32::constants::Network::Regtest,
                )
                .expect("Lightning funding tx should always be to a SegWit output")
                .to_address();

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
                            .expect("To be able to close a channel we cannot open");
                        return;
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
                for (hash, payment) in payments.iter_mut() {
                    if *hash == payment_hash {
                        payment.preimage = Some(payment_preimage);
                        payment.status = HTLCStatus::Succeeded;

                        let preimage_hash = hex::encode(payment_preimage.0);
                        tracing::info!(
                            amount_msat = ?payment.amt_msat.0,
                            fee_paid_msat = ?fee_paid_msat,
                            payment_hash = %hex::encode(payment_hash.0),
                            %preimage_hash,
                            "\nSuccessfully sent payment",
                        );
                    }
                }
            }
            Event::OpenChannelRequest { .. } => {
                // Unreachable, we don't set manually_accept_inbound_channels
            }
            Event::PaymentPathSuccessful { .. } => {}
            Event::PaymentPathFailed { .. } => {}
            Event::PaymentFailed { payment_hash, .. } => {
                print!("\nEVENT: Failed to send payment to payment hash {:?}: exhausted payment retry attempts", hex::encode(payment_hash.0));

                let mut payments = self.outbound_payments.lock().unwrap();
                if payments.contains_key(&payment_hash) {
                    let payment = payments.get_mut(&payment_hash).unwrap();
                    payment.status = HTLCStatus::Failed;
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
                        from_prev_str, to_next_str, fee_earned, from_onchain_str
                    );
                } else {
                    tracing::info!(
                        "Forwarded payment{}{}, claiming onchain {}",
                        from_prev_str, to_next_str, from_onchain_str
                    );
                }
            }
            Event::PendingHTLCsForwardable { time_forwardable } => {
                let forwarding_channel_manager = self.channel_manager.clone();
                let min = time_forwardable.as_millis() as u64;
                tokio::spawn(async move {
                    let millis_to_sleep = thread_rng().gen_range(min, min * 5);
                    tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
                    forwarding_channel_manager.process_pending_htlc_forwards();
                });
            }
            Event::SpendableOutputs { outputs } => {
                let destination_address = self.wallet.inner().get_unused_address().unwrap();
                let output_descriptors = &outputs.iter().collect::<Vec<_>>();
                let tx_feerate = self
                    .wallet
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
                let spending_tx = self
                    .keys_manager
                    .spend_spendable_outputs(
                        output_descriptors,
                        Vec::new(),
                        destination_address.script_pubkey(),
                        tx_feerate,
                        &Secp256k1::new(),
                    )
                    .unwrap();
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
            Event::ChannelReady { .. } => {}
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
                tracing::info!(%payment_hash, %amount_msat, "Received payment");

                let payment_preimage = match purpose {
                    PaymentPurpose::InvoicePayment { payment_preimage, .. } => payment_preimage,
                    PaymentPurpose::SpontaneousPayment(preimage) => Some(preimage),
                };
                self.channel_manager.claim_funds(payment_preimage.unwrap());
            }
            Event::HTLCIntercepted {
                intercept_id: _,
                requested_next_hop_scid: _,
                payment_hash: _,
                inbound_amount_msat: _,
                expected_outbound_amount_msat: _,
            } => todo!(),
        }
        })
    }
}
