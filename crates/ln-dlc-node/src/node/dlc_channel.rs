use crate::node::event::NodeEvent;
use crate::node::Node;
use crate::node::Storage as LnDlcStorage;
use crate::storage::TenTenOneStorage;
use crate::DlcMessageHandler;
use crate::PeerManager;
use crate::ToHex;
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
use dlc_manager::DlcChannelId;
use dlc_manager::Oracle;
use dlc_manager::Storage;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use time::OffsetDateTime;
use tokio::task::spawn_blocking;

impl<S: TenTenOneStorage + 'static, N: LnDlcStorage + Sync + Send + 'static> Node<S, N> {
    pub async fn propose_dlc_channel(
        &self,
        contract_input: ContractInput,
        counterparty: PublicKey,
    ) -> Result<[u8; 32]> {
        tracing::info!(
            trader_id = counterparty.to_hex(),
            oracles = ?contract_input.contract_infos[0].oracles,
            "Sending DLC channel offer"
        );

        if let Some(channel) = self
            .list_signed_dlc_channels()?
            .iter()
            .find(|channel| channel.counter_party == counterparty)
        {
            tracing::error!(
                trader_id = %counterparty,
                existing_channel_id = channel.channel_id.to_hex(),
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

                let offer_channel = dlc_manager.offer_channel(&contract_input, counterparty)?;

                let temporary_contract_id = offer_channel.temporary_contract_id;

                // TODO(holzeis): We should send the dlc message last to make sure that we have
                // finished updating the 10101 meta data before the app responds to the message.
                event_handler.publish(NodeEvent::SendDlcMessage {
                    peer: counterparty,
                    msg: Message::Channel(ChannelMessage::Offer(offer_channel)),
                })?;

                Ok(temporary_contract_id)
            }
        })
        .await?
    }

    pub fn accept_dlc_channel_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel offer");

        let (msg, _channel_id, _contract_id, counter_party) =
            self.dlc_manager.accept_channel(channel_id)?;

        self.event_handler.publish(NodeEvent::SendDlcMessage {
            peer: counter_party,
            msg: Message::Channel(ChannelMessage::Accept(msg)),
        })?;

        Ok(())
    }

    pub async fn close_dlc_channel(&self, channel_id: DlcChannelId, force: bool) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);
        tracing::info!(channel_id = channel_id_hex, "Closing DLC channel");

        let channel = self
            .get_signed_dlc_channel(|channel| channel.channel_id == channel_id)?
            .context("DLC channel to close not found")?;

        if force {
            self.force_close_dlc_channel(&channel_id)?;
        } else {
            self.propose_dlc_channel_collaborative_close(channel)
                .await?
        }

        Ok(())
    }

    fn force_close_dlc_channel(&self, channel_id: &DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(
            channel_id = %channel_id_hex,
            "Force closing DLC channel"
        );

        self.dlc_manager.force_close_channel(channel_id)?;
        Ok(())
    }

    /// Collaboratively close a DLC channel on-chain if there is no open position
    async fn propose_dlc_channel_collaborative_close(&self, channel: SignedChannel) -> Result<()> {
        let channel_id_hex = hex::encode(channel.channel_id);

        tracing::info!(
            channel_id = %channel_id_hex,
            "Closing DLC channel collaboratively"
        );

        let counterparty = channel.counter_party;

        match channel.state {
            SignedChannelState::Settled { counter_payout, .. } => {
                spawn_blocking({
                    let dlc_manager = self.dlc_manager.clone();
                    let event_handler = self.event_handler.clone();
                    move || {
                        tracing::info!(
                            counter_payout,
                            channel_id = channel.channel_id.to_hex(),
                            "Proposing collaborative close"
                        );
                        let settle_offer = dlc_manager
                            .offer_collaborative_close(&channel.channel_id, counter_payout)
                            .context(
                                "Could not propose to collaboratively close the dlc channel.",
                            )?;

                        event_handler.publish(NodeEvent::SendDlcMessage {
                            peer: counterparty,
                            msg: Message::Channel(ChannelMessage::CollaborativeCloseOffer(
                                settle_offer,
                            )),
                        })?;

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
        channel_id: DlcChannelId,
        accept_settlement_amount: u64,
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
            move || {
                let (settle_offer, counterparty) =
                    dlc_manager.settle_offer(&channel_id, accept_settlement_amount)?;

                // TODO(holzeis): We should send the dlc message last to make sure that we have
                // finished updating the 10101 meta data before the app responds to the message.
                event_handler.publish(NodeEvent::SendDlcMessage {
                    peer: counterparty,
                    msg: Message::Channel(ChannelMessage::SettleOffer(settle_offer)),
                })?;

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
            peer: counterparty_pk,
            msg: Message::Channel(ChannelMessage::SettleAccept(settle_offer)),
        })?;

        Ok(())
    }

    /// Propose an update to the DLC channel based on the provided [`ContractInput`]. A
    /// [`RenewOffer`] is sent to the counterparty, kickstarting the renew protocol.
    pub async fn propose_dlc_channel_update(
        &self,
        dlc_channel_id: &DlcChannelId,
        contract_input: ContractInput,
    ) -> Result<[u8; 32]> {
        tracing::info!(channel_id = %hex::encode(dlc_channel_id), "Proposing a DLC channel update");
        spawn_blocking({
            let dlc_manager = self.dlc_manager.clone();
            let dlc_channel_id = *dlc_channel_id;
            let event_handler = self.event_handler.clone();
            move || {
                // Not actually needed. See https://github.com/p2pderivatives/rust-dlc/issues/149.
                let counter_payout = 0;

                let (renew_offer, counterparty_pubkey) =
                    dlc_manager.renew_offer(&dlc_channel_id, counter_payout, &contract_input)?;

                // TODO(holzeis): We should send the dlc message last to make sure that we have
                // finished updating the 10101 meta data before the app responds to the message.
                event_handler.publish(NodeEvent::SendDlcMessage {
                    msg: Message::Channel(ChannelMessage::RenewOffer(renew_offer)),
                    peer: counterparty_pubkey,
                })?;

                let offered_contracts = dlc_manager.get_store().get_contract_offers()?;

                // We assume that the first `OfferedContract` we find here is the one we just
                // proposed when renewing the DLC channel.
                //
                // TODO: Change `renew_offer` API to return the `temporary_contract_id`, like
                // `offer_channel` does.
                let offered_contract = offered_contracts
                    .iter()
                    .find(|contract| contract.counter_party == counterparty_pubkey)
                    .context("Cold not find offered contract after proposing DLC channel update")?;

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
            counter_party,
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
            signed_channel.counter_party == *counterparty_pk
        })
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
                    dlc_channel_id.to_hex()
                )
            })
    }

    pub fn get_established_dlc_channel(&self, pubkey: &PublicKey) -> Result<Option<SignedChannel>> {
        let matcher = |dlc_channel: &&SignedChannel| {
            dlc_channel.counter_party == *pubkey
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

    // TODO: This API could return the number of required confirmations + the number of current
    // confirmations.
    pub fn is_dlc_channel_confirmed(&self, dlc_channel_id: &DlcChannelId) -> Result<bool> {
        let contract = self.get_contract_by_dlc_channel_id(dlc_channel_id)?;

        Ok(matches!(contract, Contract::Confirmed { .. }))
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
            _ => bail!("DLC channel {} not signed", channel_id.to_hex()),
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
}

/// Ensure that a [`dlc_messages::Message`] is sent straight away.
///
/// Use this instead of [`MessageHandler`]'s `send_message` which only enqueues the message.
///
/// [`MessageHandler`]: dlc_messages::message_handler::MessageHandler
pub fn send_dlc_message<S: TenTenOneStorage + 'static, N: LnDlcStorage + Sync + Send + 'static>(
    dlc_message_handler: &DlcMessageHandler,
    peer_manager: &PeerManager<S, N>,
    node_id: PublicKey,
    msg: Message,
) {
    // Enqueue the message.
    dlc_message_handler.send_message(node_id, msg);

    // According to the LDK docs, you don't _have_ to call this function explicitly if you are
    // using [`lightning-net-tokio`], which we are. But calling it ensures that we send the
    // enqueued message ASAP.
    peer_manager.process_events();
}
