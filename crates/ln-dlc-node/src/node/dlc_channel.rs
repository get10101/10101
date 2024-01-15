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
use anyhow::Error;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::Contract;
use dlc_manager::subchannel::SubChannel;
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
    ) -> std::result::Result<[u8; 32], Error> {
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
                "We can't open a new channel because we still have an open dlc-channel");
            bail!("Cant have more than one dlc channel.");
        }

        spawn_blocking({
            let p2pd_oracles = self.oracles.clone();

            let sub_channel_manager = self.sub_channel_manager.clone();
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

                let sub_channel_offer = sub_channel_manager
                    .get_dlc_manager()
                    .offer_channel(&contract_input, counterparty)?;

                let temporary_contract_id = sub_channel_offer.temporary_contract_id;

                if let Err(e) = event_handler.publish(NodeEvent::SendDlcMessage {
                    peer: counterparty,
                    msg: Message::Channel(ChannelMessage::Offer(sub_channel_offer)),
                }) {
                    tracing::error!("Failed to publish send dlc message node event! {e:#}");
                }

                Ok(temporary_contract_id)
            }
        })
        .await?
    }

    /// Proposes and update to the DLC channel based on the provided [`ContractInput`]. A
    /// [`RenewOffer`] is sent to the counterparty, kickstarting the renew protocol.
    pub async fn propose_dlc_channel_update(
        &self,
        dlc_channel_id: &DlcChannelId,
        payout_amount: u64,
        contract_input: ContractInput,
    ) -> Result<()> {
        tracing::info!(channel_id = %hex::encode(dlc_channel_id), "Proposing a DLC channel update");
        spawn_blocking({
            let dlc_manager = self.dlc_manager.clone();
            let dlc_channel_id = *dlc_channel_id;
            let event_handler = self.event_handler.clone();
            move || {
                let (renew_offer, counterparty_pubkey) =
                    dlc_manager.renew_offer(&dlc_channel_id, payout_amount, &contract_input)?;

                event_handler.publish(NodeEvent::SendDlcMessage {
                    peer: counterparty_pubkey,
                    msg: Message::Channel(ChannelMessage::RenewOffer(renew_offer)),
                })?;

                Ok(())
            }
        })
        .await
        .map_err(|e| anyhow!("{e:#}"))?
    }

    pub fn reject_dlc_channel_offer(&self, channel_id: &DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Rejecting DLC channel offer");

        // TODO: implement reject dlc channel offer
        // let (node_id, reject) = self
        //     .sub_channel_manager.get_dlc_manager().rejec
        //     .reject_sub_channel_offer(*channel_id)?;

        // send_dlc_message(
        //     &self.dlc_message_handler,
        //     &self.peer_manager,
        //     node_id,
        //     Message::SubChannel(SubChannelMessage::Reject(reject)),
        // );

        Ok(())
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
            SignedChannelState::Settled { .. } | SignedChannelState::RenewFinalized { .. } => {
                spawn_blocking({
                    let dlc_manager = self.dlc_manager.clone();
                    let event_handler = self.event_handler.clone();
                    move || {
                        let settle_offer = dlc_manager
                            .offer_collaborative_close(
                                &channel.channel_id,
                                channel.counter_params.collateral,
                            )
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
                tracing::error!( state = %channel.state, "Can't collaboratively close a channel with an open position.");
                bail!("Can't collaboratively close a channel with an open position");
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

                event_handler.publish(NodeEvent::SendDlcMessage {
                    peer: counterparty,
                    msg: Message::Channel(ChannelMessage::SettleOffer(settle_offer)),
                })?;

                Ok(())
            }
        })
        .await?
    }

    pub fn accept_dlc_channel_collaborative_close(&self, channel_id: DlcChannelId) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative close offer");

        let dlc_manager = self.dlc_manager.clone();
        dlc_manager.accept_collaborative_close(&channel_id)?;

        Ok(())
    }

    pub fn accept_dlc_channel_collaborative_settlement(
        &self,
        channel_id: DlcChannelId,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative settlement");

        let dlc_manager = self.dlc_manager.clone();
        let (settle_offer, counterparty_pk) = dlc_manager.accept_settle_offer(&channel_id)?;

        self.event_handler.publish(NodeEvent::SendDlcMessage {
            peer: counterparty_pk,
            msg: Message::Channel(ChannelMessage::SettleAccept(settle_offer)),
        })?;

        Ok(())
    }

    pub fn get_dlc_channel_offer(&self, pubkey: &PublicKey) -> Result<Option<SubChannel>> {
        let dlc_channel = self
            .dlc_manager
            .get_store()
            .get_offered_sub_channels()?
            .into_iter()
            .find(|dlc_channel| dlc_channel.counter_party == *pubkey);

        Ok(dlc_channel)
    }

    /// Gets the collateral and expiry for a signed contract of that given channel_id. Will return
    /// an error if the contract is not yet signed nor confirmed.
    pub fn get_collateral_and_expiry_for_confirmed_dlc_channel(
        &self,
        dlc_channel_id: DlcChannelId,
    ) -> Result<(u64, OffsetDateTime)> {
        match self.get_contract_by_dlc_channel_id(&dlc_channel_id)? {
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

                Ok((
                    contract.accepted_contract.accept_params.collateral,
                    expiry_timestamp,
                ))
            }
            state => bail!(
                "Confirmed contract not found for channel ID: {} which was in state {state:?}",
                hex::encode(dlc_channel_id)
            ),
        }
    }

    /// Gets the dlc channel by the dlc channel id
    pub fn get_dlc_channel_by_id(&self, dlc_channel_id: &DlcChannelId) -> Result<Channel> {
        self.dlc_manager
            .get_store()
            .get_channel(dlc_channel_id)
            .map_err(|e| anyhow!("{e:#}"))?
            .with_context(|| {
                format!(
                    "Couldn't find channel by id {}",
                    hex::encode(dlc_channel_id)
                )
            })
    }

    /// Fetches the contract for a given dlc channel id
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

    /// Returns the usable balance in all DLC channels. Usable means, the amount currently locked up
    /// in a position does not count to the balance
    pub fn get_usable_dlc_channel_balance(&self) -> Result<Amount> {
        let dlc_channels = self.list_signed_dlc_channels()?;
        Ok(Amount::from_sat(
            dlc_channels
                .iter()
                .map(|channel| match &channel.state {
                    SignedChannelState::Settled { own_payout, .. } => {
                        // we settled the position inside the dlc-channel
                        *own_payout
                    }

                    SignedChannelState::SettledOffered { .. }
                    | SignedChannelState::SettledReceived { .. }
                    | SignedChannelState::SettledAccepted { .. }
                    | SignedChannelState::SettledConfirmed { .. }
                    | SignedChannelState::Established { .. } => {
                        // if we are not yet settled or just established the channel, we have an
                        // open position with the full amount being locked,
                        // hence, the current balance is 0
                        0
                    }
                    SignedChannelState::RenewOffered { counter_payout, .. } => {
                        // we don't have a new position yet, but we are optimistic that it will go
                        // through. Hence, the balance is the `total money locked` minus `what the
                        // counterparty gets`
                        channel.own_params.input_amount - counter_payout
                    }
                    SignedChannelState::RenewAccepted { own_payout, .. }
                    | SignedChannelState::RenewConfirmed { own_payout, .. } => {
                        // we are currently in the phase of settling off-chain, we assume this
                        // works and take the new balance
                        *own_payout
                    }
                    SignedChannelState::RenewFinalized { own_payout, .. } => {
                        // we settled off-chain successfully
                        *own_payout
                    }
                    SignedChannelState::Closing { .. } => {
                        // the channel is almost gone, so no money left
                        0
                    }
                    SignedChannelState::CollaborativeCloseOffered { counter_payout, .. } => {
                        // the channel is not yet closed, hence, we keep showing the channel balance
                        channel.own_params.input_amount - counter_payout
                    }
                })
                .sum(),
        ))
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
