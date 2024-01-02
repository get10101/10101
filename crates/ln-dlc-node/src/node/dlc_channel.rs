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
use dlc_manager::channel::Channel;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::Contract;
use dlc_manager::subchannel::SubChannel;
use dlc_manager::DlcChannelId;
use dlc_manager::Oracle;
use dlc_manager::Storage;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use lightning::ln::ChannelId;
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

        spawn_blocking({
            let p2pd_oracles = self.oracles.clone();

            let sub_channel_manager = self.sub_channel_manager.clone();
            let oracles = contract_input.contract_infos[0].oracles.clone();
            let event_id = oracles.event_id;
            let dlc_message_handler = self.dlc_message_handler.clone();
            let peer_manager = self.peer_manager.clone();
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
                send_dlc_message(
                    &dlc_message_handler,
                    &peer_manager,
                    counterparty,
                    Message::Channel(ChannelMessage::Offer(sub_channel_offer)),
                );

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
            let dlc_message_handler = self.dlc_message_handler.clone();
            let peer_manager = self.peer_manager.clone();
            let dlc_channel_id = *dlc_channel_id;
            move || {
                let (renew_offer, counterparty_pubkey) =
                    dlc_manager.renew_offer(&dlc_channel_id, payout_amount, &contract_input)?;

                send_dlc_message(
                    &dlc_message_handler,
                    &peer_manager,
                    counterparty_pubkey,
                    Message::Channel(ChannelMessage::RenewOffer(renew_offer)),
                );
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

        send_dlc_message(
            &self.dlc_message_handler,
            &self.peer_manager,
            counter_party,
            Message::Channel(ChannelMessage::Accept(msg)),
        );

        Ok(())
    }

    pub async fn propose_dlc_channel_collaborative_settlement(
        &self,
        channel_id: ChannelId,
        accept_settlement_amount: u64,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id.0);

        tracing::info!(
            channel_id = %channel_id_hex,
            %accept_settlement_amount,
            "Settling DLC channel collaboratively"
        );

        spawn_blocking({
            let sub_channel_manager = self.sub_channel_manager.clone();
            let dlc_message_handler = self.dlc_message_handler.clone();
            let peer_manager = self.peer_manager.clone();
            move || {
                let (sub_channel_close_offer, counterparty_pk) = sub_channel_manager
                    .offer_subchannel_close(&channel_id, accept_settlement_amount)?;

                send_dlc_message(
                    &dlc_message_handler,
                    &peer_manager,
                    counterparty_pk,
                    Message::SubChannel(SubChannelMessage::CloseOffer(sub_channel_close_offer)),
                );

                Ok(())
            }
        })
        .await?
    }

    pub fn accept_dlc_channel_collaborative_settlement(
        &self,
        channel_id: &ChannelId,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id.0);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative settlement");

        let (sub_channel_close_accept, counterparty_pk) = self
            .sub_channel_manager
            .accept_subchannel_close_offer(channel_id)?;

        send_dlc_message(
            &self.dlc_message_handler,
            &self.peer_manager,
            counterparty_pk,
            Message::SubChannel(SubChannelMessage::CloseAccept(sub_channel_close_accept)),
        );

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
