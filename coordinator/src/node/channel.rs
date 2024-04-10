use crate::db;
use crate::dlc_protocol::DlcProtocolType;
use crate::dlc_protocol::ProtocolId;
use crate::node::Node;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::Txid;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::channel::ClosedChannel;
use dlc_manager::channel::ClosedPunishedChannel;
use dlc_manager::channel::ClosingChannel;
use dlc_manager::channel::SettledClosingChannel;
use dlc_manager::DlcChannelId;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_30;
use ln_dlc_node::bitcoin_conversion::to_txid_30;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_storage::DlcChannelEvent;
use time::OffsetDateTime;
use tokio::sync::broadcast::error::RecvError;

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
    pub fn spawn_shadow_dlc_channels_task(&self) {
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
            DlcChannelEvent::Closing(_) => {
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
                    &mut conn,
                    &channel.get_id(),
                    to_txid_30(buffer_transaction.txid()),
                )?;
            }
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
            DlcChannelEvent::Closed(_)
            | DlcChannelEvent::CounterClosed(_)
            | DlcChannelEvent::CollaborativelyClosed(_) => {
                let close_txid = match channel {
                    Channel::Closed(ClosedChannel { closing_txid, .. }) => closing_txid,
                    Channel::CounterClosed(ClosedChannel { closing_txid, .. }) => closing_txid,
                    Channel::CollaborativelyClosed(ClosedChannel { closing_txid, .. }) => {
                        closing_txid
                    }
                    channel => {
                        bail!("DLC channel in unexpected state. dlc_channel = {channel:?}")
                    }
                };

                db::dlc_channels::set_channel_collab_closed(
                    &mut conn,
                    &channel.get_id(),
                    to_txid_30(*close_txid),
                )?;
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
}
