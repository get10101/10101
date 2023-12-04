use crate::db;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::storage::TenTenOneNodeStorage;
use crate::trade::order;
use crate::trade::order::FailureReason;
use crate::trade::order::InvalidDlcOffer;
use crate::trade::position;
use crate::trade::position::PositionState;
use anyhow::Context;
use anyhow::Result;
use anyhow::{anyhow, bail};
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::TransactionDetails;
use bitcoin::hashes::hex::ToHex;
use bitcoin::Txid;
use commons::order_matching_fee_taker;
use dlc_messages::sub_channel::SubChannelCloseFinalize;
use dlc_messages::sub_channel::SubChannelOffer;
use dlc_messages::sub_channel::SubChannelRevoke;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use lightning::chain::transaction::OutPoint;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning::sign::DelayedPaymentOutputDescriptor;
use lightning::sign::SpendableOutputDescriptor;
use lightning::sign::StaticPaymentOutputDescriptor;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::PaymentDetails;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::transaction::Transaction;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::MillisatAmount;
use ln_dlc_node::PaymentFlow;
use ln_dlc_node::PaymentInfo;
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<node::Node<TenTenOneNodeStorage, NodeStorage>>,
    _running: Arc<RunningNode>,
    // TODO: we should make this persistent as invoices might get paid later - but for now this is
    // good enough
    pub pending_usdp_invoices: Arc<parking_lot::Mutex<HashSet<bitcoin::hashes::sha256::Hash>>>,
}

impl Node {
    pub fn new(
        node: Arc<node::Node<TenTenOneNodeStorage, NodeStorage>>,
        running: RunningNode,
    ) -> Self {
        Self {
            inner: node,
            _running: Arc::new(running),
            pending_usdp_invoices: Arc::new(Default::default()),
        }
    }
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
    pub fn get_blockchain_height(&self) -> Result<u64> {
        self.inner.get_blockchain_height()
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
        if !self
            .inner
            .dlc_message_handler
            .has_pending_messages_to_process()
        {
            return;
        }

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
                    "Failed to process DLC message: {e:#}"
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

        let resp = match &msg {
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
                        match is_expired(offer) {
                            Ok(true) => {
                                let channel_id_hex = offer.channel_id.to_hex();
                                tracing::warn!(
                                    channel_id = channel_id_hex,
                                    "Offer outdated, rejecting subchannel offer"
                                );
                                self.inner
                                    .reject_dlc_channel_offer(&channel_id)
                                    .with_context(|| {
                                        format!(
                                            "Failed to reject DLC channel offer for channel {}",
                                            hex::encode(channel_id.0)
                                        )
                                    })?;
                                order::handler::order_failed(
                                    None,
                                    FailureReason::InvalidDlcOffer(InvalidDlcOffer::Outdated),
                                    anyhow!("Outdated DLC Offer received"),
                                )
                                .context("Could not set order to failed")?;
                            }
                            Ok(false) => self
                                .inner
                                .accept_dlc_channel_offer(&channel_id)
                                .with_context(|| {
                                    format!(
                                        "Failed to accept DLC channel offer for channel {}",
                                        hex::encode(channel_id.0)
                                    )
                                })?,
                            Err(error) => {
                                bail!(error);
                            }
                        }
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
                                    hex::encode(channel_id.0)
                                )
                            })?;
                    }
                    _ => (),
                };

                resp
            }
        };

        // TODO(holzeis): It would be nice if dlc messages are also propagated via events, so the
        // receiver can decide what events to process and we can skip this component specific logic
        // here.
        if let Message::Channel(channel_message) = &msg {
            match channel_message {
                ChannelMessage::RenewOffer(r) => {
                    tracing::info!("Automatically accepting a rollover position");
                    let (accept_renew_offer, counterparty_pubkey) =
                        self.inner.dlc_manager.accept_renew_offer(&r.channel_id)?;

                    self.send_dlc_message(
                        counterparty_pubkey,
                        Message::Channel(ChannelMessage::RenewAccept(accept_renew_offer)),
                    )?;

                    let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
                        r.contract_info.get_closest_maturity_date() as i64,
                    )?;
                    position::handler::rollover_position(expiry_timestamp)?;
                }
                ChannelMessage::RenewRevoke(_) => {
                    tracing::info!("Finished rollover position");
                    // After handling the `RenewRevoke` message, we need to do some post-processing
                    // based on the fact that the DLC channel has been updated.
                    position::handler::set_position_state(PositionState::Open)?;

                    event::publish(&EventInternal::BackgroundNotification(
                        BackgroundTask::Rollover(TaskStatus::Success),
                    ));
                }
                // ignoring all other channel events.
                _ => (),
            }
        }

        // After handling the `Revoke` message, we need to do some post-processing based on the fact
        // that the DLC channel has been established
        if let Message::SubChannel(SubChannelMessage::Revoke(SubChannelRevoke {
            channel_id, ..
        })) = msg
        {
            let (accept_collateral, expiry_timestamp) = self
                .inner
                .get_collateral_and_expiry_for_confirmed_contract(channel_id)?;

            let filled_order = order::handler::order_filled()
                .context("Cannot mark order as filled for confirmed DLC")?;

            let execution_price = filled_order
                .execution_price()
                .expect("filled order to have a price");
            let open_position_fee = order_matching_fee_taker(
                filled_order.quantity,
                Decimal::try_from(execution_price)?,
            );

            position::handler::update_position_after_dlc_creation(
                filled_order,
                accept_collateral - open_position_fee.to_sat(),
                expiry_timestamp,
            )
            .context("Failed to update position after DLC creation")?;

            // Sending always a recover dlc background notification success message here as we do
            // not know if we might have reached this state after a restart. This event is only
            // received by the UI at the moment indicating that the dialog can be closed.
            // If the dialog is not open, this event would be simply ignored by the UI.
            //
            // FIXME(holzeis): We should not require that event and align the UI handling with
            // waiting for an order execution in the happy case with waiting for an
            // order execution after an in between restart. For now it was the easiest
            // to go parallel to that implementation so that we don't have to touch it.
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::RecoverDlc(TaskStatus::Success),
            ));
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
        // the fact that the DLC channel has been closed.
        if let Message::SubChannel(SubChannelMessage::CloseFinalize(SubChannelCloseFinalize {
            ..
        })) = msg
        {
            tracing::debug!(
                "Checking purpose of sending SubChannelCloseFinalize w.r.t. the position"
            );

            let positions = position::handler::get_positions()?;
            let position = positions
                .first()
                .context("Cannot find position even though we just received a SubChannelMessage")?;

            if position.position_state == PositionState::Resizing {
                tracing::debug!("Position is being resized");
            } else {
                tracing::debug!("Position is being closed");

                let filled_order = order::handler::order_filled()?;

                position::handler::update_position_after_dlc_closure(Some(filled_order))
                    .context("Failed to update position after DLC closure")?;
            }

            // Sending always a recover dlc background notification success message here as we do
            // not know if we might have reached this state after a restart. This event is only
            // received by the UI at the moment indicating that the dialog can be closed.
            // If the dialog is not open, this event would be simply ignored by the UI.
            //
            // FIXME(holzeis): We should not require that event and align the UI handling with
            // waiting for an order execution in the happy case with waiting for an
            // order execution after an in between restart. For now it was the easiest
            // to go parallel to that implementation so that we don't have to touch it.
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::RecoverDlc(TaskStatus::Success),
            ));
        };

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

/// Returns true if the contract is already expired. Errors if the contract is not a valid unix
/// timestamp
fn is_expired(offer: &SubChannelOffer) -> Result<bool> {
    let now = OffsetDateTime::now_utc();
    let offer_expiry =
        OffsetDateTime::from_unix_timestamp(offer.contract_info.get_closest_maturity_date() as i64)
            .context("Could not convert maturity date into offset date time")?;
    Ok(offer_expiry.lt(&now))
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
        fee_msat: MillisatAmount,
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
        funding_txid: Option<Txid>,
    ) -> Result<()> {
        match db::get_payment(*payment_hash)? {
            Some(_) => {
                db::update_payment(
                    *payment_hash,
                    htlc_status,
                    amt_msat,
                    fee_msat,
                    preimage,
                    secret,
                    funding_txid,
                )?;
            }
            None => {
                db::insert_payment(
                    *payment_hash,
                    PaymentInfo {
                        preimage,
                        secret,
                        status: htlc_status,
                        amt_msat,
                        fee_msat,
                        flow,
                        timestamp: OffsetDateTime::now_utc(),
                        description: "".to_string(),
                        invoice: None,
                        funding_txid,
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

    fn get_announced_channel(&self, counterparty_pubkey: PublicKey) -> Result<Option<Channel>> {
        db::get_announced_channel(counterparty_pubkey)
    }

    fn get_channel_by_payment_hash(&self, payment_hash: String) -> Result<Option<Channel>> {
        db::get_channel_by_payment_hash(payment_hash)
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

#[cfg(test)]
pub mod tests {
    use crate::ln_dlc::node::is_expired;
    use bitcoin::secp256k1::schnorr::Signature;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::Script;
    use dlc_messages::contract_msgs::ContractInfo;
    use dlc_messages::contract_msgs::ContractInfoInner;
    use dlc_messages::contract_msgs::SingleContractInfo;
    use dlc_messages::oracle_msgs::DigitDecompositionEventDescriptor;
    use dlc_messages::oracle_msgs::EventDescriptor;
    use dlc_messages::oracle_msgs::OracleAnnouncement;
    use dlc_messages::oracle_msgs::OracleEvent;
    use dlc_messages::oracle_msgs::OracleInfo;
    use dlc_messages::oracle_msgs::SingleOracleInfo;
    use dlc_messages::sub_channel::SubChannelOffer;
    use lightning::ln::ChannelId;
    use ln_dlc_node::node::rust_dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
    use ln_dlc_node::node::rust_dlc_manager::contract::ContractDescriptor;
    use ln_dlc_node::node::rust_dlc_manager::payout_curve::PayoutFunction;
    use ln_dlc_node::node::rust_dlc_manager::payout_curve::PayoutFunctionPiece;
    use ln_dlc_node::node::rust_dlc_manager::payout_curve::PayoutPoint;
    use ln_dlc_node::node::rust_dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
    use ln_dlc_node::node::rust_dlc_manager::payout_curve::RoundingInterval;
    use ln_dlc_node::node::rust_dlc_manager::payout_curve::RoundingIntervals;
    use secp256k1_zkp::XOnlyPublicKey;
    use std::str::FromStr;
    use time::Duration;
    use time::OffsetDateTime;

    #[test]
    pub fn contract_with_maturity_in_past_is_expired() {
        // setup
        let expired_timestamp =
            (OffsetDateTime::now_utc() - Duration::seconds(10)).unix_timestamp() as u32;

        let contract = create_dummy_contract();
        let offer = create_dummy_offer(contract, expired_timestamp);

        // act
        let is_expired = is_expired(&offer).unwrap();

        // assert
        assert!(is_expired)
    }
    #[test]
    pub fn contract_with_maturity_in_future_is_valid() {
        // setup
        let expired_timestamp =
            (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp() as u32;

        let contract = create_dummy_contract();
        let offer = create_dummy_offer(contract, expired_timestamp);

        // act
        let is_expired = is_expired(&offer).unwrap();

        // assert
        assert!(!is_expired)
    }

    fn create_dummy_contract() -> ContractDescriptor {
        ContractDescriptor::Numerical(NumericalDescriptor {
            payout_function: PayoutFunction::new(vec![
                PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                    PolynomialPayoutCurvePiece::new(vec![
                        PayoutPoint {
                            event_outcome: 0,
                            outcome_payout: 0,
                            extra_precision: 0,
                        },
                        PayoutPoint {
                            event_outcome: 50_000,
                            outcome_payout: 0,
                            extra_precision: 0,
                        },
                    ])
                    .unwrap(),
                ),
            ])
            .unwrap(),
            rounding_intervals: RoundingIntervals {
                intervals: vec![RoundingInterval {
                    begin_interval: 0,
                    rounding_mod: 10,
                }],
            },
            difference_params: None,
            oracle_numeric_infos: dlc_trie::OracleNumericInfo {
                base: 2,
                nb_digits: vec![20],
            },
        })
    }

    fn create_dummy_offer(descriptor: ContractDescriptor, maturity_epoch: u32) -> SubChannelOffer {
        let random_pk = PublicKey::from_str(
            "0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166",
        )
        .unwrap();
        SubChannelOffer {
            channel_id: ChannelId([0u8; 32]),
            next_per_split_point: random_pk,
            revocation_basepoint: random_pk,
            publish_basepoint: random_pk,
            own_basepoint: random_pk,
            channel_own_basepoint: random_pk,
            channel_publish_basepoint: random_pk,
            channel_revocation_basepoint: random_pk,
            contract_info: ContractInfo::SingleContractInfo(SingleContractInfo {
                total_collateral: 0,
                contract_info: ContractInfoInner {
                    contract_descriptor: (&descriptor).into(),
                    oracle_info: OracleInfo::Single(SingleOracleInfo {
                        oracle_announcement: OracleAnnouncement {
                            announcement_signature: dummy_signature(),
                            oracle_public_key: dummy_oraclye_x_only_pk(),
                            oracle_event: OracleEvent {
                                oracle_nonces: vec![],
                                event_maturity_epoch: maturity_epoch,
                                event_descriptor: EventDescriptor::DigitDecompositionEvent(
                                    DigitDecompositionEventDescriptor {
                                        base: 2,
                                        is_signed: false,
                                        unit: "kg/sats".to_string(),
                                        precision: 1,
                                        nb_digits: 10,
                                    },
                                ),
                                event_id: "dummy".to_string(),
                            },
                        },
                    }),
                },
            }),
            channel_first_per_update_point: random_pk,
            payout_spk: Script::default(),
            payout_serial_id: 123,
            offer_collateral: 123,
            cet_locktime: 140,
            refund_locktime: 1337,
            cet_nsequence: 100,
            fee_rate_per_vbyte: 1,
        }
    }

    fn dummy_signature() -> Signature {
        Signature::from_str("6470FD1303DDA4FDA717B9837153C24A6EAB377183FC438F939E0ED2B620E9EE5077C4A8B8DCA28963D772A94F5F0DDF598E1C47C137F91933274C7C3EDADCE8").unwrap()
    }

    fn dummy_oraclye_x_only_pk() -> XOnlyPublicKey {
        XOnlyPublicKey::from_str("16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0")
            .unwrap()
    }
}
