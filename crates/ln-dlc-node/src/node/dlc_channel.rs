use crate::bitcoin_conversion::to_secp_pk_29;
use crate::bitcoin_conversion::to_secp_pk_30;
use crate::node::event::NodeEvent;
use crate::node::Node;
use crate::node::Storage as LnDlcStorage;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use crate::DlcMessageHandler;
use crate::PeerManager;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::Contract;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::ContractId;
use dlc_manager::DlcChannelId;
use dlc_manager::Oracle;
use dlc_manager::ReferenceId;
use dlc_manager::Storage;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use time::OffsetDateTime;
use tokio::task::spawn_blocking;

impl<D: BdkStorage, S: TenTenOneStorage + 'static, N: LnDlcStorage + Sync + Send + 'static>
    Node<D, S, N>
{
    pub async fn propose_dlc_channel(
        &self,
        contract_input: ContractInput,
        counterparty: PublicKey,
        protocol_id: ReferenceId,
    ) -> Result<(ContractId, DlcChannelId)> {
        tracing::info!(
            trader_id = %counterparty,
            oracles = ?contract_input.contract_infos[0].oracles,
            "Sending DLC channel offer"
        );

        if let Some(channel) = self
            .list_signed_dlc_channels()?
            .iter()
            .find(|channel| channel.counter_party == to_secp_pk_29(counterparty))
        {
            tracing::error!(
                trader_id = %counterparty,
                existing_channel_id = hex::encode(channel.channel_id),
                existing_channel_state = %channel.state,
                "We can't open a new channel because we still have an open dlc-channel"
            );
            bail!("Cant have more than one dlc channel.");
        }

        spawn_blocking({
            let p2pd_oracles = self.oracles.clone();

            let dlc_manager = self.dlc_manager.clone();
            let oracles = contract_input.contract_infos[0].oracles.clone();
            let event_id = oracles.event_id;
            let event_handler = self.event_handler.clone();
            move || {
                let announcements: Vec<_> = p2pd_oracles
                    .into_iter()
                    .filter(|o| oracles.public_keys.contains(&o.public_key))
                    .filter_map(|oracle| oracle.get_announcement(&event_id).ok())
                    .collect();

                ensure!(
                    !announcements.is_empty(),
                    format!("Can't propose dlc channel without oracles")
                );

                let offer_channel = dlc_manager.offer_channel(
                    &contract_input,
                    to_secp_pk_29(counterparty),
                    Some(protocol_id),
                )?;

                let temporary_contract_id = offer_channel.temporary_contract_id;
                let temporary_channel_id = offer_channel.temporary_channel_id;

                event_handler.publish(NodeEvent::StoreDlcMessage {
                    peer: counterparty,
                    msg: Message::Channel(ChannelMessage::Offer(offer_channel)),
                });

                Ok((temporary_contract_id, temporary_channel_id))
            }
        })
        .await?
    }

    #[cfg(test)]
    pub fn accept_dlc_channel_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel offer");

        let (msg, _channel_id, _contract_id, counter_party) =
            self.dlc_manager.accept_channel(channel_id)?;

        self.event_handler.publish(NodeEvent::SendDlcMessage {
            peer: to_secp_pk_30(counter_party),
            msg: Message::Channel(ChannelMessage::Accept(msg)),
        });

        Ok(())
    }

    pub async fn close_dlc_channel(
        &self,
        channel_id: DlcChannelId,
        is_force_close: bool,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(
            is_force_close,
            channel_id = channel_id_hex,
            "Closing DLC channel"
        );

        let channel = self
            .get_signed_dlc_channel(|channel| channel.channel_id == channel_id)?
            .context("DLC channel to close not found")?;

        if is_force_close {
            self.force_close_dlc_channel(channel)?;
        } else {
            self.propose_dlc_channel_collaborative_close(channel)
                .await?
        }

        Ok(())
    }

    fn force_close_dlc_channel(&self, channel: SignedChannel) -> Result<()> {
        let channel_id = channel.channel_id;
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(
            channel_id = %channel_id_hex,
            "Force closing DLC channel"
        );

        self.dlc_manager
            .force_close_channel(&channel_id, channel.reference_id)?;
        Ok(())
    }

    /// Close a DLC channel on-chain collaboratively, if there is no open position.
    async fn propose_dlc_channel_collaborative_close(&self, channel: SignedChannel) -> Result<()> {
        let counterparty = channel.counter_party;

        match channel.state {
            SignedChannelState::Settled { counter_payout, .. } => {
                spawn_blocking({
                    let dlc_manager = self.dlc_manager.clone();
                    let event_handler = self.event_handler.clone();
                    move || {
                        tracing::info!(
                            counter_payout,
                            channel_id = hex::encode(channel.channel_id),
                            "Proposing collaborative close"
                        );

                        let settle_offer = dlc_manager
                            .offer_collaborative_close(
                                &channel.channel_id,
                                counter_payout,
                                channel.reference_id,
                            )
                            .context(
                                "Could not propose to collaboratively close the dlc channel.",
                            )?;

                        event_handler.publish(NodeEvent::SendDlcMessage {
                            peer: to_secp_pk_30(counterparty),
                            msg: Message::Channel(ChannelMessage::CollaborativeCloseOffer(
                                settle_offer,
                            )),
                        });

                        anyhow::Ok(())
                    }
                })
                .await??;
            }
            _ => {
                tracing::error!( state = %channel.state, "Can't collaboratively close a channel which is not settled.");
                bail!("Can't collaboratively close a channel which is not settled");
            }
        }

        Ok(())
    }

    /// Collaboratively close a position within a DLC Channel
    pub async fn propose_dlc_channel_collaborative_settlement(
        &self,
        channel_id: &DlcChannelId,
        accept_settlement_amount: u64,
        protocol_id: ReferenceId,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(
            channel_id = %channel_id_hex,
            %accept_settlement_amount,
            "Settling DLC in channel collaboratively"
        );

        spawn_blocking({
            let dlc_manager = self.dlc_manager.clone();
            let event_handler = self.event_handler.clone();
            let channel_id = *channel_id;
            move || {
                let (settle_offer, counterparty) = dlc_manager.settle_offer(
                    &channel_id,
                    accept_settlement_amount,
                    Some(protocol_id),
                )?;

                event_handler.publish(NodeEvent::StoreDlcMessage {
                    peer: to_secp_pk_30(counterparty),
                    msg: Message::Channel(ChannelMessage::SettleOffer(settle_offer)),
                });

                Ok(())
            }
        })
        .await?
    }

    pub fn accept_dlc_channel_collaborative_close(&self, channel_id: &DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative close offer");

        let dlc_manager = self.dlc_manager.clone();
        dlc_manager.accept_collaborative_close(channel_id)?;

        Ok(())
    }

    pub fn accept_dlc_channel_collaborative_settlement(
        &self,
        channel_id: &DlcChannelId,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative settlement");

        let dlc_manager = self.dlc_manager.clone();
        let (settle_offer, counterparty_pk) = dlc_manager.accept_settle_offer(channel_id)?;

        self.event_handler.publish(NodeEvent::SendDlcMessage {
            peer: to_secp_pk_30(counterparty_pk),
            msg: Message::Channel(ChannelMessage::SettleAccept(settle_offer)),
        });

        Ok(())
    }

    /// Propose an update to the DLC channel based on the provided [`ContractInput`]. A
    /// [`RenewOffer`] is sent to the counterparty, kickstarting the renew protocol.
    pub async fn propose_dlc_channel_update(
        &self,
        dlc_channel_id: &DlcChannelId,
        contract_input: ContractInput,
        protocol_id: ReferenceId,
    ) -> Result<ContractId> {
        tracing::info!(channel_id = %hex::encode(dlc_channel_id), "Proposing a DLC channel update");
        spawn_blocking({
            let dlc_manager = self.dlc_manager.clone();
            let dlc_channel_id = *dlc_channel_id;
            let event_handler = self.event_handler.clone();
            move || {
                // Not actually needed. See https://github.com/p2pderivatives/rust-dlc/issues/149.
                let counter_payout = 0;

                let (renew_offer, counterparty_pubkey) = dlc_manager.renew_offer(
                    &dlc_channel_id,
                    counter_payout,
                    &contract_input,
                    Some(protocol_id),
                )?;

                event_handler.publish(NodeEvent::StoreDlcMessage {
                    msg: Message::Channel(ChannelMessage::RenewOffer(renew_offer)),
                    peer: to_secp_pk_30(counterparty_pubkey),
                });

                let offered_contracts = dlc_manager.get_store().get_contract_offers()?;

                // We assume that the first `OfferedContract` we find here is the one we just
                // proposed when renewing the DLC channel.
                //
                // TODO: Change `renew_offer` API to return the `temporary_contract_id`, like
                // `offer_channel` does.
                let offered_contract = offered_contracts
                    .iter()
                    .find(|contract| contract.counter_party == counterparty_pubkey)
                    .context(
                        "Could not find offered contract after proposing DLC channel update",
                    )?;

                Ok(offered_contract.id)
            }
        })
        .await
        .map_err(|e| anyhow!("{e:#}"))?
    }

    #[cfg(test)]
    /// Accept an update to the DLC channel. This can only succeed if we previously received a DLC
    /// channel update offer from the the counterparty.
    // The accept code has diverged on the app side (hence the #[cfg(test)]). Another hint that we
    // should delete most of this crate soon.
    pub fn accept_dlc_channel_update(&self, channel_id: &DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel update offer");

        let (msg, counter_party) = self.dlc_manager.accept_renew_offer(channel_id)?;

        send_dlc_message(
            &self.dlc_message_handler,
            &self.peer_manager,
            to_secp_pk_30(counter_party),
            Message::Channel(ChannelMessage::RenewAccept(msg)),
        );

        Ok(())
    }

    /// Get the expiry for the [`SignedContract`] corresponding to the given [`DlcChannelId`].
    ///
    /// Will return an error if the contract is not yet signed or confirmed on-chain.
    pub fn get_expiry_for_confirmed_dlc_channel(
        &self,
        dlc_channel_id: &DlcChannelId,
    ) -> Result<OffsetDateTime> {
        match self.get_contract_by_dlc_channel_id(dlc_channel_id)? {
            Contract::Signed(contract) | Contract::Confirmed(contract) => {
                let offered_contract = contract.accepted_contract.offered_contract;
                let contract_info = offered_contract
                    .contract_info
                    .first()
                    .expect("contract info to exist on a signed contract");
                let oracle_announcement = contract_info
                    .oracle_announcements
                    .first()
                    .expect("oracle announcement to exist on signed contract");

                let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
                    oracle_announcement.oracle_event.event_maturity_epoch as i64,
                )?;

                Ok(expiry_timestamp)
            }
            state => bail!(
                "Confirmed contract not found for channel ID: {} which was in state {state:?}",
                hex::encode(dlc_channel_id)
            ),
        }
    }

    pub fn get_dlc_channel_by_reference_id(&self, reference_id: ReferenceId) -> Result<Channel> {
        let channels = self.list_dlc_channels()?;
        channels
            .into_iter()
            .find(|channel| channel.get_reference_id() == Some(reference_id))
            .context("Couldn't find channel by reference id")
    }

    /// Get the DLC [`Channel`] by its [`DlcChannelId`].
    pub fn get_dlc_channel_by_id(&self, dlc_channel_id: &DlcChannelId) -> Result<Channel> {
        self.dlc_manager
            .get_store()
            .get_channel(dlc_channel_id)?
            .with_context(|| {
                format!(
                    "Couldn't find channel by id {}",
                    hex::encode(dlc_channel_id)
                )
            })
    }

    pub fn get_signed_dlc_channel_by_counterparty(
        &self,
        counterparty_pk: &PublicKey,
    ) -> Result<Option<SignedChannel>> {
        self.get_signed_dlc_channel(|signed_channel| {
            signed_channel.counter_party == to_secp_pk_29(*counterparty_pk)
        })
    }

    pub fn get_contract_by_id(&self, contract_id: &ContractId) -> Result<Option<Contract>> {
        let contract = self.dlc_manager.get_store().get_contract(contract_id)?;
        Ok(contract)
    }

    /// Fetch the [`Contract`] corresponding to the given [`DlcChannelId`].
    pub fn get_contract_by_dlc_channel_id(
        &self,
        dlc_channel_id: &DlcChannelId,
    ) -> Result<Contract> {
        let channel = self.get_dlc_channel_by_id(dlc_channel_id)?;
        let contract_id = channel
            .get_contract_id()
            .context("Could not find contract id")?;

        self.dlc_manager
            .get_store()
            .get_contract(&contract_id)?
            .with_context(|| {
                format!(
                    "Couldn't find dlc channel with id: {}",
                    hex::encode(dlc_channel_id)
                )
            })
    }

    pub fn get_established_dlc_channel(&self, pubkey: &PublicKey) -> Result<Option<SignedChannel>> {
        let matcher = |dlc_channel: &&SignedChannel| {
            dlc_channel.counter_party == to_secp_pk_29(*pubkey)
                && matches!(&dlc_channel.state, SignedChannelState::Established { .. })
        };
        let dlc_channel = self.get_signed_dlc_channel(&matcher)?;
        Ok(dlc_channel)
    }

    fn get_signed_dlc_channel(
        &self,
        matcher: impl FnMut(&&SignedChannel) -> bool,
    ) -> Result<Option<SignedChannel>> {
        let dlc_channels = self.list_signed_dlc_channels()?;
        let dlc_channel = dlc_channels.iter().find(matcher);

        Ok(dlc_channel.cloned())
    }

    pub fn list_signed_dlc_channels(&self) -> Result<Vec<SignedChannel>> {
        let dlc_channels = self.dlc_manager.get_store().get_signed_channels(None)?;

        Ok(dlc_channels)
    }

    pub fn list_dlc_channels(&self) -> Result<Vec<Channel>> {
        let dlc_channels = self.dlc_manager.get_store().get_channels()?;

        Ok(dlc_channels)
    }

    pub fn is_signed_dlc_channel_confirmed_by_trader_id(
        &self,
        trader_id: PublicKey,
    ) -> Result<bool> {
        let signed_channel = self.get_signed_channel_by_trader_id(trader_id)?;
        self.is_dlc_channel_confirmed(&signed_channel.channel_id)
    }

    // TODO: This API could return the number of required confirmations + the number of current
    // confirmations.
    pub fn is_dlc_channel_confirmed(&self, dlc_channel_id: &DlcChannelId) -> Result<bool> {
        let channel = self.get_dlc_channel_by_id(dlc_channel_id)?;
        let confirmed = match channel {
            Channel::Signed(signed_channel) => match signed_channel.state {
                SignedChannelState::Established {
                    signed_contract_id, ..
                } => {
                    let contract = self.get_contract_by_id(&signed_contract_id)?.context(
                        "Could not find contract for signed channel in state Established.",
                    )?;
                    matches!(contract, Contract::Confirmed { .. })
                }
                _ => true,
            },
            Channel::Offered(_)
            | Channel::Accepted(_)
            | Channel::FailedAccept(_)
            | Channel::FailedSign(_)
            | Channel::Cancelled(_) => false,
            Channel::Closing(_)
            | Channel::SettledClosing(_)
            | Channel::Closed(_)
            | Channel::CounterClosed(_)
            | Channel::ClosedPunished(_)
            | Channel::CollaborativelyClosed(_) => true,
        };

        Ok(confirmed)
    }

    /// Return the usable balance for all the DLC channels.
    pub fn get_dlc_channels_usable_balance(&self) -> Result<Amount> {
        self.list_signed_dlc_channels()?
            .iter()
            .try_fold(Amount::ZERO, |acc, channel| {
                let balance = self.get_dlc_channel_usable_balance(&channel.channel_id)?;

                Ok(acc + balance)
            })
    }

    /// Return the usable counterparty balance for all the DLC channels.
    pub fn get_dlc_channels_usable_balance_counterparty(&self) -> Result<Amount> {
        self.list_signed_dlc_channels()?
            .iter()
            .try_fold(Amount::ZERO, |acc, channel| {
                let balance =
                    self.get_dlc_channel_usable_balance_counterparty(&channel.channel_id)?;

                Ok(acc + balance)
            })
    }

    pub fn signed_dlc_channel_total_collateral(&self, channel_id: &DlcChannelId) -> Result<Amount> {
        let channel = self.get_dlc_channel_by_id(channel_id)?;

        match channel {
            Channel::Signed(channel) => Ok(Amount::from_sat(
                channel.own_params.collateral + channel.counter_params.collateral,
            )),
            _ => bail!("DLC channel {} not signed", hex::encode(channel_id)),
        }
    }

    /// Return the usable balance for the DLC channel.
    ///
    /// Usable balance excludes all balance which is being wagered in DLCs. It also excludes some
    /// reserved funds to be used when the channel is closed on-chain.
    pub fn get_dlc_channel_usable_balance(&self, channel_id: &DlcChannelId) -> Result<Amount> {
        let dlc_channel = self.get_dlc_channel_by_id(channel_id)?;

        let usable_balance = match dlc_channel {
            Channel::Signed(SignedChannel {
                state: SignedChannelState::Settled { own_payout, .. },
                ..
            }) => {
                // We settled the position inside the DLC channel.
                Amount::from_sat(own_payout)
            }
            Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledOffered { counter_payout, .. },
                own_params,
                counter_params,
                ..
            })
            | Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledReceived { counter_payout, .. },
                own_params,
                counter_params,
                ..
            })
            | Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledAccepted { counter_payout, .. },
                own_params,
                counter_params,
                ..
            })
            | Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledConfirmed { counter_payout, .. },
                own_params,
                counter_params,
                ..
            }) => {
                // We haven't settled the DLC off-chain yet, but we are optimistic that the
                // protocol will complete. Hence, the usable balance is the
                // total collateral minus what the counterparty gets.
                Amount::from_sat(own_params.collateral + counter_params.collateral - counter_payout)
            }
            Channel::Signed(SignedChannel {
                state: SignedChannelState::CollaborativeCloseOffered { counter_payout, .. },
                own_params,
                counter_params,
                ..
            }) => {
                // The channel is not yet closed. Hence, we keep showing the channel balance.
                Amount::from_sat(own_params.collateral + counter_params.collateral - counter_payout)
            }
            // For all other cases we can rely on the `Contract`, since
            // `SignedChannelState::get_contract_id` will return a `ContractId` for
            // them.
            _ => self.get_contract_own_usable_balance(&dlc_channel)?,
        };

        Ok(usable_balance)
    }

    /// Return the usable balance for the DLC channel, for the counterparty.
    ///
    /// Usable balance excludes all balance which is being wagered in DLCs. It also excludes some
    /// reserved funds to be used when the channel is closed on-chain.
    pub fn get_dlc_channel_usable_balance_counterparty(
        &self,
        channel_id: &DlcChannelId,
    ) -> Result<Amount> {
        let dlc_channel = self.get_dlc_channel_by_id(channel_id)?;

        let usable_balance = match dlc_channel {
            Channel::Signed(SignedChannel {
                state: SignedChannelState::Settled { counter_payout, .. },
                ..
            }) => {
                // We settled the position inside the DLC channel.
                Amount::from_sat(counter_payout)
            }
            Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledOffered { counter_payout, .. },
                ..
            })
            | Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledReceived { counter_payout, .. },
                ..
            })
            | Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledAccepted { counter_payout, .. },
                ..
            })
            | Channel::Signed(SignedChannel {
                state: SignedChannelState::SettledConfirmed { counter_payout, .. },
                ..
            }) => {
                // We haven't settled the DLC off-chain yet, but we are optimistic that the
                // protocol will complete.
                Amount::from_sat(counter_payout)
            }
            Channel::Signed(SignedChannel {
                state: SignedChannelState::CollaborativeCloseOffered { counter_payout, .. },
                ..
            }) => {
                // The channel is not yet closed.
                Amount::from_sat(counter_payout)
            }
            // For all other cases we can rely on the `Contract`, since
            // `SignedChannelState::get_contract_id` will return a `ContractId` for
            // them.
            _ => self.get_contract_counterparty_usable_balance(&dlc_channel)?,
        };

        Ok(usable_balance)
    }

    fn get_contract_own_usable_balance(&self, dlc_channel: &Channel) -> Result<Amount> {
        self.get_contract_usable_balance(dlc_channel, true)
    }

    fn get_contract_counterparty_usable_balance(&self, dlc_channel: &Channel) -> Result<Amount> {
        self.get_contract_usable_balance(dlc_channel, false)
    }

    fn get_contract_usable_balance(
        &self,
        dlc_channel: &Channel,
        is_balance_being_calculated_for_self: bool,
    ) -> Result<Amount> {
        let contract_id = match dlc_channel.get_contract_id() {
            Some(contract_id) => contract_id,
            None => return Ok(Amount::ZERO),
        };

        let contract = self
            .dlc_manager
            .get_store()
            .get_contract(&contract_id)
            .context("Could not find contract associated with channel to compute usable balance")?
            .context("Could not find contract associated with channel to compute usable balance")?;

        // We are only including contracts that are actually established.
        //
        // TODO: Model other kinds of balance (e.g. pending incoming, pending outgoing)
        // to avoid situations where money appears to be missing.
        let signed_contract = match contract {
            Contract::Signed(signed_contract) | Contract::Confirmed(signed_contract) => {
                signed_contract
            }
            _ => return Ok(Amount::ZERO),
        };

        let am_i_offer_party = signed_contract
            .accepted_contract
            .offered_contract
            .is_offer_party;

        let is_balance_being_calculated_for_offer_party = if is_balance_being_calculated_for_self {
            am_i_offer_party
        }
        // If we want the counterparty balance, their role in the protocol (offer or accept) is the
        // opposite of ours.
        else {
            !am_i_offer_party
        };

        let offered_contract = signed_contract.accepted_contract.offered_contract;

        let total_collateral = offered_contract.total_collateral;

        let usable_balance = match &offered_contract.contract_info[0].contract_descriptor {
            ContractDescriptor::Enum(_) => {
                unreachable!("We are not using DLCs with enumerated outcomes");
            }
            ContractDescriptor::Numerical(descriptor) => {
                let payouts = descriptor
                    .get_payouts(total_collateral)
                    .expect("valid payouts");

                // The minimum payout for each party determines how many coins are _not_ currently
                // being wagered. Since they are not being wagered, they have the potential to be
                // wagered (by renewing the channel, for example) and so they are usable.
                let reserve = if is_balance_being_calculated_for_offer_party {
                    payouts
                        .iter()
                        .min_by(|a, b| a.offer.cmp(&b.offer))
                        .expect("at least one")
                        .offer
                } else {
                    payouts
                        .iter()
                        .min_by(|a, b| a.accept.cmp(&b.accept))
                        .expect("at least one")
                        .accept
                };

                Amount::from_sat(reserve)
            }
        };

        Ok(usable_balance)
    }

    #[cfg(test)]
    pub fn process_incoming_messages(&self) -> Result<()> {
        use crate::node::dlc_message_name;

        let dlc_message_handler = &self.dlc_message_handler;
        let dlc_manager = &self.dlc_manager;
        let peer_manager = &self.peer_manager;
        let messages = dlc_message_handler.get_and_clear_received_messages();
        tracing::debug!("Received and cleared {} messages", messages.len());

        for (node_id, msg) in messages {
            tracing::info!(
                from = %to_secp_pk_30(node_id),
                msg = %dlc_message_name(&msg),
                "Processing rust-dlc message"
            );

            match msg {
                Message::OnChain(_) | Message::Channel(_) => {
                    let resp = dlc_manager.on_dlc_message(&msg, node_id)?;

                    if let Some(msg) = resp {
                        tracing::debug!(to = %to_secp_pk_30(node_id), msg = dlc_message_name(&msg), "Sending DLC-manager message");
                        send_dlc_message(
                            dlc_message_handler,
                            peer_manager,
                            to_secp_pk_30(node_id),
                            msg,
                        );
                    }
                }
                Message::SubChannel(_) => {
                    tracing::error!("Not sending subchannel message");
                }
            }
        }

        Ok(())
    }

    // Rollback the channel to the last "stable" state. Note, this is potentially risky to do as the
    // counterparty may still old signed transactions, that would allow them to punish us if we were
    // to publish an outdated transaction.
    pub fn roll_back_channel(&self, signed_channel: &SignedChannel) -> Result<()> {
        let mut signed_channel = signed_channel.clone();

        let state = signed_channel
            .clone()
            .roll_back_state
            .context("Missing rollback state")?;

        signed_channel.state = state;
        self.dlc_manager
            .get_store()
            .upsert_channel(Channel::Signed(signed_channel), None)?;

        Ok(())
    }
}

/// Ensure that a [`dlc_messages::Message`] is sent straight away.
///
/// Use this instead of [`MessageHandler`]'s `send_message` which only enqueues the message.
///
/// [`MessageHandler`]: dlc_messages::message_handler::MessageHandler
pub fn send_dlc_message<D: BdkStorage, S: TenTenOneStorage + 'static, N: LnDlcStorage>(
    dlc_message_handler: &DlcMessageHandler,
    peer_manager: &PeerManager<D, S, N>,
    node_id: PublicKey,
    msg: Message,
) {
    // Enqueue the message.
    dlc_message_handler.send_message(to_secp_pk_29(node_id), msg);

    // According to the LDK docs, you don't _have_ to call this function explicitly if you are
    // using [`lightning-net-tokio`], which we are. But calling it ensures that we send the
    // enqueued message ASAP.
    peer_manager.process_events();
}

/// Give an estimate for the fee reserve of a DLC channel, given a fee rate.
///
/// Limitations:
///
/// - `rust-dlc` assumes that both parties will use P2WPKH script pubkeys for their CET outputs. If
/// they don't then the reserved fee might be slightly over or under the target fee rate.
///
/// - Rounding errors can cause very slight differences between what we estimate here and what
/// `rust-dlc` will end up reserving.
pub fn estimated_dlc_channel_fee_reserve(fee_rate_sats_per_vb: f64) -> Amount {
    let buffer_weight_wu = dlc::channel::BUFFER_TX_WEIGHT;

    let cet_or_refund_weight_wu = {
        let cet_or_refund_base_weight_wu = dlc::CET_BASE_WEIGHT;
        // Because the CET spends from a buffer transaction, compared to a regular DLC that spends
        // directly from the funding transaction.
        let cet_or_refund_extra_weight_wu = dlc::channel::CET_EXTRA_WEIGHT;

        // This is the standard length of a P2WPKH script pubkey.
        let cet_or_refund_output_spk_bytes = 22;

        // Value = 8 bytes; var_int = 1 byte.
        let cet_or_refund_output_weight_wu = (8 + 1 + cet_or_refund_output_spk_bytes) * 4;

        cet_or_refund_base_weight_wu
            + cet_or_refund_extra_weight_wu
            // 1 output per party.
            + (2 * cet_or_refund_output_weight_wu)
    };

    let total_weight_vb = (buffer_weight_wu + cet_or_refund_weight_wu) as f64 / 4.0;

    let total_fee_reserve = total_weight_vb * fee_rate_sats_per_vb;
    let total_fee_reserve = total_fee_reserve.ceil() as u64;

    Amount::from_sat(total_fee_reserve)
}

/// Give an estimate for the fee paid to publish a DLC channel funding transaction, given a fee
/// rate.
///
/// This estimate is based on a funding transaction spending _two_ P2WPKH inputs (one per party) and
/// including _two_ P2WPKH change outputs (also one per party).
///
/// Values taken from
/// https://github.com/discreetlogcontracts/dlcspecs/blob/master/Transactions.md#fees.
pub fn estimated_funding_transaction_fee(fee_rate_sats_per_vb: f64) -> Amount {
    let base_weight_wu = dlc::FUND_TX_BASE_WEIGHT;

    let input_script_pubkey_length = 22;
    let max_witness_length = 108;
    let input_weight_wu = 164 + (4 * input_script_pubkey_length) + max_witness_length;

    let output_script_pubkey_length = 22;
    let output_weight_wu = 36 + (4 * output_script_pubkey_length);

    let total_weight_wu = base_weight_wu + (input_weight_wu * 2) + (output_weight_wu * 2);
    let total_weight_vb = total_weight_wu as f64 / 4.0;

    let fee = total_weight_vb * fee_rate_sats_per_vb;
    let fee = fee.ceil() as u64;

    Amount::from_sat(fee)
}
