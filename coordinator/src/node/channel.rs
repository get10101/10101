use crate::db;
use crate::dlc_protocol;
use crate::dlc_protocol::DlcProtocolType;
use crate::node::Node;
use crate::position::models::PositionState;
use crate::FundingFee;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::ScriptBuf;
use bitcoin::Txid;
use bitcoin_old::Transaction;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::channel::ClosedChannel;
use dlc_manager::channel::ClosedPunishedChannel;
use dlc_manager::channel::ClosingChannel;
use dlc_manager::channel::SettledClosingChannel;
use dlc_manager::contract::ClosedContract;
use dlc_manager::contract::Contract;
use dlc_manager::contract::PreClosedContract;
use dlc_manager::DlcChannelId;
use dlc_manager::ReferenceId;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::sync::broadcast::error::RecvError;
use xxi_node::bitcoin_conversion::to_secp_pk_30;
use xxi_node::bitcoin_conversion::to_txid_30;
use xxi_node::node::event::NodeEvent;
use xxi_node::node::ProtocolId;
use xxi_node::storage::DlcChannelEvent;

pub enum DlcChannelState {
    Pending,
    Open,
    Closing,
    Closed,
    Failed,
    Cancelled,
}

pub struct DlcChannel {
    pub channel_id: DlcChannelId,
    pub trader: PublicKey,
    pub channel_state: DlcChannelState,
    pub trader_reserve_sats: Amount,
    pub coordinator_reserve_sats: Amount,
    pub coordinator_funding_sats: Amount,
    pub trader_funding_sats: Amount,
    pub funding_txid: Option<Txid>,
    pub close_txid: Option<Txid>,
    pub settle_txid: Option<Txid>,
    pub buffer_txid: Option<Txid>,
    pub claim_txid: Option<Txid>,
    pub punish_txid: Option<Txid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Node {
    pub async fn force_close_dlc_channel(&self, channel_id: DlcChannelId) -> Result<()> {
        self.inner.close_dlc_channel(channel_id, true).await?;
        Ok(())
    }

    pub async fn close_dlc_channel(&self, channel_id: DlcChannelId) -> Result<()> {
        let channel = self.inner.get_dlc_channel_by_id(&channel_id)?;
        let previous_id = channel
            .get_reference_id()
            .map(ProtocolId::try_from)
            .transpose()?;

        let protocol_id = self.inner.close_dlc_channel(channel_id, false).await?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());

        protocol_executor.start_close_channel_protocol(
            protocol_id,
            previous_id,
            &channel.get_id(),
            &to_secp_pk_30(channel.get_counter_party_id()),
        )?;

        Ok(())
    }

    pub fn spawn_watch_dlc_channel_events_task(&self) {
        let mut receiver = self.inner.event_handler.subscribe();

        tokio::spawn({
            let node = self.clone();
            async move {
                loop {
                    match receiver.recv().await {
                        Ok(NodeEvent::DlcChannelEvent { dlc_channel_event }) => {
                            if let Err(e) = node.process_dlc_channel_event(dlc_channel_event) {
                                tracing::error!(
                                    ?dlc_channel_event,
                                    "Failed to process DLC channel event. Error: {e:#}"
                                );
                            }
                        }
                        Ok(NodeEvent::Connected { .. })
                        | Ok(NodeEvent::SendDlcMessage { .. })
                        | Ok(NodeEvent::StoreDlcMessage { .. })
                        | Ok(NodeEvent::SendLastDlcMessage { .. }) => {} // ignored
                        Err(RecvError::Lagged(skipped)) => {
                            tracing::warn!("Skipped {skipped} messages");
                        }
                        Err(RecvError::Closed) => {
                            tracing::error!("Lost connection to sender!");
                            break;
                        }
                    }
                }
            }
        });
    }

    pub fn process_dlc_channel_event(&self, dlc_channel_event: DlcChannelEvent) -> Result<()> {
        let mut conn = self.pool.get()?;

        let protocol_id = match dlc_channel_event.get_reference_id() {
            Some(reference_id) => reference_id,
            None => {
                bail!("Can't process dlc channel event without reference id. dlc_channel_event = {dlc_channel_event:?}");
            }
        };

        if let DlcChannelEvent::Deleted(_) = dlc_channel_event {
            // we need to handle the delete event here, as the corresponding channel isn't existing
            // anymore.
            let protocol_id = ProtocolId::try_from(protocol_id)?;
            db::dlc_channels::set_channel_failed(&mut conn, &protocol_id)?;
            return Ok(());
        }

        let channel = &self.inner.get_dlc_channel_by_reference_id(protocol_id)?;

        match dlc_channel_event {
            DlcChannelEvent::Offered(_) => {
                let open_protocol_id = ProtocolId::try_from(protocol_id)?;
                db::dlc_channels::insert_pending_dlc_channel(
                    &mut conn,
                    &open_protocol_id,
                    &channel.get_id(),
                    &to_secp_pk_30(channel.get_counter_party_id()),
                )?;
            }
            DlcChannelEvent::Established(_) | DlcChannelEvent::Settled(_) => {
                let signed_channel = match channel {
                    Channel::Signed(signed_channel) => signed_channel,
                    channel => {
                        bail!("Dlc channel in unexpected state. dlc_channel = {channel:?}");
                    }
                };

                let trader_reserve = self
                    .inner
                    .get_dlc_channel_usable_balance_counterparty(&signed_channel.channel_id)?;
                let coordinator_reserve = self
                    .inner
                    .get_dlc_channel_usable_balance(&signed_channel.channel_id)?;

                let coordinator_funding = Amount::from_sat(signed_channel.own_params.collateral);
                let trader_funding = Amount::from_sat(signed_channel.counter_params.collateral);

                let protocol_id = ProtocolId::try_from(protocol_id)?;
                let dlc_protocol = db::dlc_protocols::get_dlc_protocol(&mut conn, protocol_id)?;

                match dlc_protocol.protocol_type {
                    DlcProtocolType::OpenChannel { .. } => {
                        db::dlc_channels::set_dlc_channel_open(
                            &mut conn,
                            &protocol_id,
                            &channel.get_id(),
                            to_txid_30(signed_channel.fund_tx.txid()),
                            coordinator_reserve,
                            trader_reserve,
                            coordinator_funding,
                            trader_funding,
                        )?;
                    }
                    DlcProtocolType::OpenPosition { .. }
                    | DlcProtocolType::Settle { .. }
                    | DlcProtocolType::Rollover { .. }
                    | DlcProtocolType::ResizePosition { .. } => {
                        db::dlc_channels::update_channel(
                            &mut conn,
                            &channel.get_id(),
                            coordinator_reserve,
                            trader_reserve,
                        )?;
                    }
                    DlcProtocolType::Close { .. } | DlcProtocolType::ForceClose { .. } => {} /* ignored */
                }
            }
            DlcChannelEvent::SettledClosing(_) => {
                let (settle_transaction, claim_transaction) = match channel {
                    Channel::Signed(SignedChannel {
                        state:
                            SignedChannelState::SettledClosing {
                                settle_transaction, ..
                            },
                        ..
                    }) => (settle_transaction, None),
                    Channel::SettledClosing(SettledClosingChannel {
                        settle_transaction,
                        claim_transaction,
                        ..
                    }) => (settle_transaction, Some(claim_transaction)),
                    channel => {
                        bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
                    }
                };

                db::dlc_channels::set_channel_force_closing_settled(
                    &mut conn,
                    &channel.get_id(),
                    to_txid_30(settle_transaction.txid()),
                    claim_transaction.map(|tx| to_txid_30(tx.txid())),
                )?;
            }
            DlcChannelEvent::Closing(_) => self.handle_closing_event(&mut conn, channel)?,
            DlcChannelEvent::ClosedPunished(_) => {
                let punish_txid = match channel {
                    Channel::ClosedPunished(ClosedPunishedChannel { punish_txid, .. }) => {
                        punish_txid
                    }
                    channel => {
                        bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
                    }
                };

                db::dlc_channels::set_channel_punished(
                    &mut conn,
                    &channel.get_id(),
                    to_txid_30(*punish_txid),
                )?;
            }
            DlcChannelEvent::CollaborativeCloseOffered(_) => {
                let close_transaction = match channel {
                    Channel::Signed(SignedChannel {
                        state: SignedChannelState::CollaborativeCloseOffered { close_tx, .. },
                        ..
                    }) => close_tx,
                    channel => {
                        bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
                    }
                };

                db::dlc_channels::set_channel_collab_closing(
                    &mut conn,
                    &channel.get_id(),
                    to_txid_30(close_transaction.txid()),
                )?;
            }
            DlcChannelEvent::Closed(_) | DlcChannelEvent::CounterClosed(_) => {
                self.handle_force_closed_event(&mut conn, channel, protocol_id)?
            }
            DlcChannelEvent::CollaborativelyClosed(_) => {
                self.handle_collaboratively_closed_event(&mut conn, channel, protocol_id)?
            }
            DlcChannelEvent::FailedAccept(_) | DlcChannelEvent::FailedSign(_) => {
                let protocol_id = ProtocolId::try_from(protocol_id)?;
                db::dlc_channels::set_channel_failed(&mut conn, &protocol_id)?;
            }
            DlcChannelEvent::Cancelled(_) => {
                let protocol_id = ProtocolId::try_from(protocol_id)?;
                db::dlc_channels::set_channel_cancelled(&mut conn, &protocol_id)?;
            }
            DlcChannelEvent::Deleted(_) => {} // delete is handled above.
            DlcChannelEvent::Accepted(_)
            | DlcChannelEvent::SettledOffered(_)
            | DlcChannelEvent::SettledReceived(_)
            | DlcChannelEvent::SettledAccepted(_)
            | DlcChannelEvent::SettledConfirmed(_)
            | DlcChannelEvent::RenewOffered(_)
            | DlcChannelEvent::RenewAccepted(_)
            | DlcChannelEvent::RenewConfirmed(_)
            | DlcChannelEvent::RenewFinalized(_) => {} // intermediate state changes are ignored
        }

        Ok(())
    }

    pub fn apply_funding_fee_to_channel(
        &self,
        dlc_channel_id: DlcChannelId,
        funding_fee: FundingFee,
    ) -> Result<(Amount, Amount)> {
        let collateral_reserve_coordinator =
            self.inner.get_dlc_channel_usable_balance(&dlc_channel_id)?;
        let collateral_reserve_trader = self
            .inner
            .get_dlc_channel_usable_balance_counterparty(&dlc_channel_id)?;

        // The party earning the funding fee receives adds it to their collateral reserve.
        // Conversely, the party paying the funding fee subtracts it from their margin.
        let reserves = match funding_fee {
            FundingFee::Zero => (collateral_reserve_coordinator, collateral_reserve_trader),
            FundingFee::CoordinatorPays(funding_fee) => {
                let funding_fee = funding_fee.to_signed().expect("to fit");

                let collateral_reserve_trader =
                    collateral_reserve_trader.to_signed().expect("to fit");
                let new_collateral_reserve_trader = collateral_reserve_trader + funding_fee;
                let new_collateral_reserve_trader =
                    new_collateral_reserve_trader.to_unsigned().expect("to fit");

                (
                    // The coordinator pays the funding fee using their margin. Thus, their
                    // collateral reserve remains unchanged.
                    collateral_reserve_coordinator,
                    new_collateral_reserve_trader,
                )
            }
            FundingFee::TraderPays(funding_fee) => {
                let funding_fee = funding_fee.to_signed().expect("to fit");

                let collateral_reserve_coordinator =
                    collateral_reserve_coordinator.to_signed().expect("to fit");
                let new_collateral_reserve_coordinator =
                    collateral_reserve_coordinator + funding_fee;
                let new_collateral_reserve_coordinator = new_collateral_reserve_coordinator
                    .to_unsigned()
                    .expect("to fit");

                (
                    new_collateral_reserve_coordinator,
                    // The trader pays the funding fee using their margin. Thus, their
                    // collateral reserve remains unchanged.
                    collateral_reserve_trader,
                )
            }
        };

        Ok(reserves)
    }

    fn handle_closing_event(&self, conn: &mut PgConnection, channel: &Channel) -> Result<()> {
        // If a channel is set to closing it means the buffer transaction got broadcasted,
        // which will only happen if the channel got force closed while the
        // user had an open position.
        let trader_id = channel.get_counter_party_id();

        // we do not know the price yet, since we have to wait for the position to expire.
        if db::positions::Position::set_open_position_to_closing(
            conn,
            &to_secp_pk_30(trader_id),
            None,
        )? > 0
        {
            tracing::info!(%trader_id, "Set open position to closing after the dlc channel got force closed.");
        }

        let buffer_transaction = match channel {
            Channel::Signed(SignedChannel {
                state:
                    SignedChannelState::Closing {
                        buffer_transaction, ..
                    },
                ..
            }) => buffer_transaction,
            Channel::Closing(ClosingChannel {
                buffer_transaction, ..
            }) => buffer_transaction,
            channel => {
                bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
            }
        };

        db::dlc_channels::set_channel_force_closing(
            conn,
            &channel.get_id(),
            to_txid_30(buffer_transaction.txid()),
        )?;

        Ok(())
    }

    fn handle_force_closed_event(
        &self,
        conn: &mut PgConnection,
        channel: &Channel,
        reference_id: ReferenceId,
    ) -> Result<()> {
        let protocol_id = ProtocolId::try_from(reference_id)?;
        let dlc_protocol = db::dlc_protocols::get_dlc_protocol(conn, protocol_id)?;
        let contract_id = &dlc_protocol.contract_id.context("Missing contract id")?;
        let trader_id = dlc_protocol.trader;
        let contract = self
            .inner
            .get_contract_by_id(contract_id)?
            .context("Missing contract")?;

        let position = db::positions::Position::get_position_by_trader(
            conn,
            trader_id,
            /* the closing price doesn't matter here. */
            vec![PositionState::Closing { closing_price: 0.0 }],
        )?
        .with_context(|| {
            format!("Couldn't find closing position for trader. trader_id = {trader_id}")
        })?;

        let (closing_price, trader_realized_pnl_sat) = match contract {
            Contract::PreClosed(PreClosedContract {
                // We assume a closed contract does always have an attestation
                attestations: Some(attestations),
                signed_cet,
                ..
            })
            | Contract::Closed(ClosedContract {
                // We assume a closed contract does always have an attestation
                attestations: Some(attestations),
                signed_cet: Some(signed_cet),
                ..
            }) => {
                let trader_realized_pnl_sat = self.calculate_trader_realized_pnl_from_cet(
                    conn,
                    &dlc_protocol.channel_id,
                    signed_cet,
                )?;

                let closing_price = Decimal::from_str_radix(
                    &attestations
                        .first()
                        .context("at least one attestation")?
                        .outcomes
                        .join(""),
                    2,
                )?;

                (closing_price, trader_realized_pnl_sat)
            }
            contract => {
                bail!("Contract in unexpected state. Expected PreClosed or Closed Got: {:?}, trader_id = {trader_id}", contract)
            }
        };

        tracing::debug!(
            ?position,
            %trader_id,
            "Finalize closing position after force closure",
        );

        if db::positions::Position::set_position_to_closed_with_pnl(
            conn,
            position.id,
            trader_realized_pnl_sat,
            closing_price,
        )? > 0
        {
            tracing::info!(%trader_id, "Set closing position to closed after the dlc channel got force closed.");
        } else {
            tracing::warn!(%trader_id, "Failed to set closing position to closed after the dlc channel got force closed.");
        }

        let close_txid = match channel {
            Channel::Closed(ClosedChannel { closing_txid, .. }) => closing_txid,
            Channel::CounterClosed(ClosedChannel { closing_txid, .. }) => closing_txid,
            channel => {
                bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
            }
        };

        db::dlc_channels::set_channel_closed(conn, &channel.get_id(), to_txid_30(*close_txid))?;
        Ok(())
    }

    fn handle_collaboratively_closed_event(
        &self,
        conn: &mut PgConnection,
        channel: &Channel,
        reference_id: ReferenceId,
    ) -> Result<()> {
        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
        protocol_executor.finish_dlc_protocol(
            ProtocolId::try_from(reference_id)?,
            &to_secp_pk_30(channel.get_counter_party_id()),
            None,
            &channel.get_id(),
            self.tx_position_feed.clone(),
        )?;

        let close_txid = match channel {
            Channel::CollaborativelyClosed(ClosedChannel { closing_txid, .. }) => closing_txid,
            channel => {
                bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
            }
        };

        db::dlc_channels::set_channel_closed(conn, &channel.get_id(), to_txid_30(*close_txid))?;

        Ok(())
    }

    /// Calculates the trader realized pnl from the cet outputs which do not belong to us.
    /// 1. Sum the trader payouts
    /// 2. Subtract the trader reserve sats from the trader payout
    fn calculate_trader_realized_pnl_from_cet(
        &self,
        conn: &mut PgConnection,
        channel_id: &DlcChannelId,
        signed_cet: Transaction,
    ) -> Result<i64> {
        let trader_payout: u64 = signed_cet
            .output
            .iter()
            .filter(|output| {
                !self
                    .inner
                    .is_mine(&ScriptBuf::from_bytes(output.script_pubkey.to_bytes()))
            })
            .map(|output| output.value)
            .sum();

        let dlc_channel =
            db::dlc_channels::get_dlc_channel(conn, channel_id)?.with_context(|| {
                format!("Couldn't find dlc channel by channel id = {:?}", channel_id)
            })?;

        let trader_realized_pnl_sat =
            trader_payout as i64 - dlc_channel.trader_reserve_sats.to_sat() as i64;

        Ok(trader_realized_pnl_sat)
    }
}
