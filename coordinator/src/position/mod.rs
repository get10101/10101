use crate::db;
use crate::dlc_protocol::ProtocolId;
use crate::node::Node;
use crate::position::models::PositionState;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::ScriptBuf;
use bitcoin_old::Transaction;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc_manager::contract::ClosedContract;
use dlc_manager::contract::Contract;
use dlc_manager::contract::PreClosedContract;
use dlc_manager::DlcChannelId;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_30;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_storage::DlcChannelEvent;
use rust_decimal::Decimal;
use tokio::sync::broadcast::error::RecvError;

pub mod models;

impl Node {
    pub fn spawn_watch_closing_channels(&self) {
        let mut receiver = self.inner.event_handler.subscribe();

        tokio::spawn({
            let node = self.clone();
            async move {
                loop {
                    match receiver.recv().await {
                        Ok(NodeEvent::DlcChannelEvent { dlc_channel_event }) => {
                            if let Err(e) = node
                                .update_position_after_dlc_channel_event(dlc_channel_event)
                                .await
                            {
                                tracing::error!(?dlc_channel_event, "Failed to update position after dlc channel event. Error: {e:}")
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

    /// Checks if the dlc channel got force closed and updates a potential open position. If the dlc
    /// channel is closing the position will be set to `Closing`, if the dlc channel is closed or
    /// counter closed the closing position will be set to closed with a closing price (from the
    /// attestation and a trader realized pnl calculated from the cet payout and the last trader
    /// reserve)
    async fn update_position_after_dlc_channel_event(
        &self,
        dlc_channel_event: DlcChannelEvent,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;

        let reference_id = dlc_channel_event.get_reference_id().with_context(|| format!("Can't process dlc channel event without reference id. dlc_channel_event = {dlc_channel_event:?}"))?;
        let protocol_id = ProtocolId::try_from(reference_id)?;

        match dlc_channel_event {
            // If a channel is set to closing it means the buffer transaction got broadcasted, which
            // will only happen if the channel got force closed while the user had an open position.
            DlcChannelEvent::Closing(_) => {
                let channel = &self.inner.get_dlc_channel_by_reference_id(reference_id)?;
                let trader_id = channel.get_counter_party_id();

                // we do not know the price yet, since we have to wait for the position to expire.
                if db::positions::Position::set_open_position_to_closing(
                    &mut conn,
                    &to_secp_pk_30(trader_id),
                    None,
                )? > 0
                {
                    tracing::info!(%trader_id, "Set open position to closing after the dlc channel got force closed.");
                }
            }
            // A dlc channel is set to `Closed` or `CounterClosed` if the CET got broadcasted. The
            // underlying contract is either `PreClosed` or `Closed` depending on the CET
            // confirmations.
            DlcChannelEvent::Closed(_) | DlcChannelEvent::CounterClosed(_) => {
                let dlc_protocol = db::dlc_protocols::get_dlc_protocol(&mut conn, protocol_id)?;
                let trader_id = dlc_protocol.trader;
                let contract = self
                    .inner
                    .get_contract_by_id(&dlc_protocol.contract_id)?
                    .context("Missing contract")?;

                let position = db::positions::Position::get_position_by_trader(
                    &mut conn,
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
                            &mut conn,
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
                    &mut conn,
                    position.id,
                    trader_realized_pnl_sat,
                    closing_price,
                )? > 0
                {
                    tracing::info!(%trader_id, "Set closing position to closed after the dlc channel got force closed.");
                } else {
                    tracing::warn!(%trader_id, "Failed to set closing position to closed after the dlc channel got force closed.");
                }
            }
            DlcChannelEvent::Offered(_)
            | DlcChannelEvent::Accepted(_)
            | DlcChannelEvent::Established(_)
            | DlcChannelEvent::SettledOffered(_)
            | DlcChannelEvent::SettledReceived(_)
            | DlcChannelEvent::SettledAccepted(_)
            | DlcChannelEvent::SettledConfirmed(_)
            | DlcChannelEvent::Settled(_)
            | DlcChannelEvent::SettledClosing(_)
            | DlcChannelEvent::RenewOffered(_)
            | DlcChannelEvent::RenewAccepted(_)
            | DlcChannelEvent::RenewConfirmed(_)
            | DlcChannelEvent::RenewFinalized(_)
            | DlcChannelEvent::CollaborativeCloseOffered(_)
            | DlcChannelEvent::ClosedPunished(_)
            | DlcChannelEvent::CollaborativelyClosed(_)
            | DlcChannelEvent::FailedAccept(_)
            | DlcChannelEvent::FailedSign(_)
            | DlcChannelEvent::Cancelled(_)
            | DlcChannelEvent::Deleted(_) => {} // ignored
        }

        Ok(())
    }

    /// Calculates the trader realized pnl from the cet outputs which do not belong to us.
    /// 1. Sum the trader payouts
    /// 2. Subtract the trader reserve sats from the trader payout
    fn calculate_trader_realized_pnl_from_cet(
        &self,
        conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
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
