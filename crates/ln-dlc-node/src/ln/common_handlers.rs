use super::event_handler::PendingInterceptedHtlcs;
use crate::channel::Channel;
use crate::channel::FakeScid;
use crate::channel::UserChannelId;
use crate::config::CONFIRMATION_TARGET;
use crate::config::HTLC_INTERCEPTED_CONNECTION_TIMEOUT;
use crate::config::LIQUIDITY_MULTIPLIER;
use crate::ln::event_handler::InterceptionDetails;
use crate::node::invoice::HTLCStatus;
use crate::node::ChannelManager;
use crate::node::Node;
use crate::node::Storage;
use crate::util;
use crate::MillisatAmount;
use crate::PaymentFlow;
use crate::PaymentInfo;
use crate::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT_MAX;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::subchannel::LNChannelManager;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::ln::channelmanager::InterceptId;
use lightning::ln::PaymentHash;
use lightning::routing::gossip::NodeId;
use lightning::util::events::PaymentPurpose;
use rand::thread_rng;
use rand::Rng;
use secp256k1_zkp::Secp256k1;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::task::block_in_place;
use uuid::Uuid;

pub(crate) fn handle_payment_claimable(
    channel_manager: &Arc<ChannelManager>,
    payment_hash: PaymentHash,
    purpose: PaymentPurpose,
    amount_msat: u64,
) -> Result<()> {
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
    channel_manager.claim_funds(preimage);
    Ok(())
}

pub(crate) fn handle_htlc_handling_failed(
    prev_channel_id: [u8; 32],
    failed_next_destination: lightning::util::events::HTLCDestination,
) {
    tracing::info!(
        prev_channel_id = %prev_channel_id.to_hex(),
        failed_next_destination = ?failed_next_destination,
        "HTLC handling failed"
    );
}

pub(crate) fn handle_discard_funding(transaction: bitcoin::Transaction, channel_id: [u8; 32]) {
    let tx_hex = serialize_hex(&transaction);
    tracing::info!(
        channel_id = %channel_id.to_hex(),
        %tx_hex,
        "Discarding funding transaction"
    );

    // FIXME: Address the comment below
    // A "real" node should probably "lock" the UTXOs spent in funding transactions
    // until the funding transaction either confirms, or this event is
    // generated.
}

pub(crate) fn handle_payment_forwarded<S>(
    node: &Arc<Node<S>>,
    prev_channel_id: Option<[u8; 32]>,
    next_channel_id: Option<[u8; 32]>,
    claim_from_onchain_tx: bool,
    fee_earned_msat: Option<u64>,
) {
    let read_only_network_graph = node.network_graph.read_only();
    let nodes = read_only_network_graph.nodes();
    let channels = node.channel_manager.list_channels();

    let node_str = |channel_id: &Option<[u8; 32]>| match channel_id {
        None => String::new(),
        Some(channel_id) => match channels.iter().find(|c| c.channel_id == *channel_id) {
            None => String::new(),
            Some(channel) => match nodes.get(&NodeId::from_pubkey(&channel.counterparty.node_id)) {
                None => " from private node".to_string(),
                Some(node) => match &node.announcement_info {
                    None => " from unnamed node".to_string(),
                    Some(announcement) => {
                        format!("node {}", announcement.alias)
                    }
                },
            },
        },
    };
    let channel_str = |channel_id: &Option<[u8; 32]>| {
        channel_id
            .map(|channel_id| format!(" with channel {}", channel_id.to_hex()))
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

pub fn handle_payment_sent<S>(
    node: &Arc<Node<S>>,
    payment_hash: PaymentHash,
    payment_preimage: lightning::ln::PaymentPreimage,
    fee_paid_msat: Option<u64>,
) -> Result<()>
where
    S: Storage,
{
    let storage = &node.storage;
    let amount_msat = match storage.get_payment(&payment_hash) {
        Ok(Some((_, PaymentInfo { amt_msat, .. }))) => {
            let amount_msat = MillisatAmount(None);
            if let Err(e) = storage.merge_payment(
                &payment_hash,
                PaymentFlow::Outbound,
                amount_msat,
                HTLCStatus::Succeeded,
                Some(payment_preimage),
                None,
            ) {
                anyhow::bail!(
                    "Failed to update sent payment: {e:#}, hash: {payment_hash}",
                    payment_hash = payment_hash.0.to_hex(),
                );
            }
            amt_msat
        }
        Ok(None) => {
            tracing::warn!("Got PaymentSent event without matching outbound payment on record");

            let amt_msat = MillisatAmount(None);
            if let Err(e) = storage.insert_payment(
                payment_hash,
                PaymentInfo {
                    preimage: Some(payment_preimage),
                    secret: None,
                    status: HTLCStatus::Succeeded,
                    amt_msat,
                    flow: PaymentFlow::Outbound,
                    timestamp: OffsetDateTime::now_utc(),
                    description: "".to_string(),
                },
            ) {
                tracing::error!(
                    payment_hash = %payment_hash.0.to_hex(),
                    "Failed to insert sent payment: {e:#}"
                );
                // TODO: Should we bail here too?
            }

            amt_msat
        }
        Err(e) => {
            anyhow::bail!(
                        "Failed to verify payment state before handling sent payment: {e:#}, hash: {payment_hash}",
                            payment_hash = payment_hash.0.to_hex(),
                    );
        }
    };
    tracing::info!(
        amount_msat = ?amount_msat.0,
        fee_paid_msat = ?fee_paid_msat,
        payment_hash = %payment_hash.0.to_hex(),
        preimage_hash = %payment_preimage.0.to_hex(),
        "Successfully sent payment",
    );
    Ok(())
}

pub(crate) fn handle_channel_closed<S>(
    node: &Arc<Node<S>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    user_channel_id: u128,
    reason: lightning::util::events::ClosureReason,
    channel_id: [u8; 32],
) -> Result<(), anyhow::Error>
where
    S: Storage,
{
    block_in_place(|| {
        let user_channel_id = Uuid::from_u128(user_channel_id).to_string();
        tracing::info!(
            %user_channel_id,
            channel_id = %channel_id.to_hex(),
            ?reason,
            "Channel closed",
        );

        if let Some(channel) = node.storage.get_channel(&user_channel_id)? {
            let counterparty = channel.counterparty;

            let channel = Channel::close_channel(channel, reason);
            node.storage.upsert_channel(channel)?;

            // Fail intercepted HTLC which was meant to be used to open the JIT channel,
            // in case it was still pending
            if let Some(interception) = pending_intercepted_htlcs.lock().get(&counterparty) {
                fail_intercepted_htlc(&node.channel_manager, &interception.id);
            }
        }

        node.sub_channel_manager
            .notify_ln_channel_closed(channel_id)?;

        anyhow::Ok(())
    })?;
    Ok(())
}

pub(crate) fn handle_spendable_outputs<S>(
    node: &Arc<Node<S>>,
    outputs: Vec<SpendableOutputDescriptor>,
) -> Result<()>
where
    S: Storage,
{
    let ldk_outputs = outputs
        .iter()
        .filter(|output| {
            // `StaticOutput`s are sent to the node's on-chain wallet directly
            !matches!(output, SpendableOutputDescriptor::StaticOutput { .. })
        })
        .collect::<Vec<_>>();
    if ldk_outputs.is_empty() {
        return Ok(());
    }
    for spendable_output in ldk_outputs.iter() {
        if let Err(e) = node
            .storage
            .insert_spendable_output((*spendable_output).clone())
        {
            tracing::error!("Failed to persist spendable output: {e:#}")
        }
    }
    let destination_script = node.wallet.inner().get_last_unused_address()?;
    let tx_feerate = node
        .fee_rate_estimator
        .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
    let spending_tx = node.keys_manager.spend_spendable_outputs(
        &ldk_outputs,
        vec![],
        destination_script.script_pubkey(),
        tx_feerate,
        &Secp256k1::new(),
    )?;
    node.wallet.broadcast_transaction(&spending_tx);
    Ok(())
}

pub(crate) fn handle_payment_claimed<S>(
    node: &Arc<Node<S>>,
    amount_msat: u64,
    payment_hash: PaymentHash,
    purpose: PaymentPurpose,
) where
    S: Storage,
{
    tracing::info!(
        %amount_msat,
        payment_hash = %payment_hash.0.to_hex(),
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
    if let Err(e) = node.storage.merge_payment(
        &payment_hash,
        PaymentFlow::Inbound,
        amount_msat,
        HTLCStatus::Succeeded,
        payment_preimage,
        payment_secret,
    ) {
        tracing::error!(
            payment_hash = %payment_hash.0.to_hex(),
            "Failed to update claimed payment: {e:#}"
        )
    }
}

pub(crate) fn handle_payment_failed<S>(node: &Arc<Node<S>>, payment_hash: PaymentHash)
where
    S: Storage,
{
    tracing::warn!(
        payment_hash = %payment_hash.0.to_hex(),
        "Failed to send payment to payment hash: exhausted payment retry attempts",
    );

    let amount_msat = MillisatAmount(None);
    if let Err(e) = node.storage.merge_payment(
        &payment_hash,
        PaymentFlow::Outbound,
        amount_msat,
        HTLCStatus::Failed,
        None,
        None,
    ) {
        tracing::error!(
            payment_hash = %payment_hash.0.to_hex(),
            "Failed to update failed payment: {e:#}"
        )
    }
}

pub(crate) async fn handle_funding_generation_ready<S>(
    node: &Arc<Node<S>>,
    user_channel_id: u128,
    counterparty_node_id: PublicKey,
    output_script: bitcoin::Script,
    channel_value_satoshis: u64,
    temporary_channel_id: [u8; 32],
) -> Result<(), anyhow::Error> {
    let user_channel_id = Uuid::from_u128(user_channel_id).to_string();
    tracing::info!(
        %user_channel_id,
        %counterparty_node_id,
        "Funding generation ready for channel with counterparty {}",
        counterparty_node_id
    );
    let target_blocks = CONFIRMATION_TARGET;
    let funding_tx_result = node
        .wallet
        .inner()
        .create_funding_transaction(output_script, channel_value_satoshis, target_blocks)
        .await;
    let funding_tx = match funding_tx_result {
        Ok(funding_tx) => funding_tx,
        Err(err) => {
            tracing::error!(
                %err,
                "Cannot open channel due to not being able to create funding tx"
            );
            node.channel_manager
                .close_channel(&temporary_channel_id, &counterparty_node_id)
                .map_err(|e| anyhow!("{e:?}"))?;

            return Ok(());
        }
    };
    if let Err(err) = node.channel_manager.funding_transaction_generated(
        &temporary_channel_id,
        &counterparty_node_id,
        funding_tx,
    ) {
        tracing::error!(?err, "Channel went away before we could fund it. The peer disconnected or refused the channel");
    };
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

pub(crate) fn handle_channel_ready<S>(
    node: &Arc<Node<S>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    user_channel_id: u128,
    channel_id: [u8; 32],
    counterparty_node_id: PublicKey,
) -> Result<()>
where
    S: Storage,
{
    block_in_place(|| {
        let res = handle_channel_ready_internal(
            node,
            pending_intercepted_htlcs,
            user_channel_id,
            channel_id,
            counterparty_node_id,
        );

        if let Err(ref e) = res {
            tracing::error!("Failed to handle ChannelReady event: {e:#}");

            // If the `ChannelReady` event was associated with a pending intercepted HTLC, we
            // must fail it to unlock the funds of all the nodes along the
            // payment route
            if let Some(interception) = pending_intercepted_htlcs.lock().get(&counterparty_node_id)
            {
                fail_intercepted_htlc(&node.channel_manager, &interception.id);
            }
        }

        res
    })
}

pub(crate) fn handle_channel_ready_internal<S>(
    node: &Arc<Node<S>>,
    pending_intercepted_htlcs: &PendingInterceptedHtlcs,
    user_channel_id: u128,
    channel_id: [u8; 32],
    counterparty_node_id: PublicKey,
) -> Result<()>
where
    S: Storage,
{
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

    let channel = node.storage.get_channel(&user_channel_id)?;
    let channel = Channel::open_channel(channel, channel_details)?;
    node.storage.upsert_channel(channel)?;

    if let Some(interception) = pending_intercepted_htlcs.lock().get(&counterparty_node_id) {
        tracing::info!(
            intercept_id = %interception.id.0.to_hex(),
            counterparty = %counterparty_node_id.to_string(),
            forward_amount_msat = %interception.expected_outbound_amount_msat,
            "Pending intercepted HTLC found, forwarding payment"
        );

        node.channel_manager
            .forward_intercepted_htlc(
                interception.id,
                &channel_id,
                counterparty_node_id,
                interception.expected_outbound_amount_msat,
            )
            .map_err(|e| anyhow!("{e:?}"))
            .context("Failed to forward intercepted HTLC")?;
    }

    Ok(())
}

/// Fail an intercepted HTLC backwards.
pub(crate) fn fail_intercepted_htlc(
    channel_manager: &Arc<ChannelManager>,
    intercept_id: &InterceptId,
) {
    tracing::error!(
        intercept_id = %intercept_id.0.to_hex(),
        "Failing intercepted HTLC backwards"
    );

    // This call fails if the HTLC was already forwarded of if the HTLC was already failed. In
    // both cases we don't have to do anything else
    let _ = channel_manager.fail_intercepted_htlc(*intercept_id);
}

pub(crate) fn handle_pending_htlcs_forwardable(
    forwarding_channel_manager: Arc<ChannelManager>,
    time_forwardable: Duration,
) {
    tracing::debug!(
        time_forwardable = ?time_forwardable,
        "Pending HTLCs are forwardable"
    );
    let min = time_forwardable.as_millis() as u64;
    tokio::spawn(async move {
        let millis_to_sleep = thread_rng().gen_range(min..(min * 5));
        tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
        forwarding_channel_manager.process_pending_htlc_forwards();
    });
}
