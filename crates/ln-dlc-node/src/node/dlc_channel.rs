use crate::node::Node;
use crate::DlcMessageHandler;
use crate::SubChannelManager;
use crate::ToHex;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::ClosedContract;
use dlc_manager::contract::Contract;
use dlc_manager::subchannel::SubChannel;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::ChannelId;
use dlc_manager::ContractId;
use dlc_manager::Oracle;
use dlc_manager::Storage;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::OnChainMessage;
use dlc_messages::SubChannelMessage;
use lightning::ln::channelmanager::ChannelDetails;
use std::sync::Arc;
use tokio::task::spawn_blocking;

impl<P> Node<P>
where
    P: Send + Sync,
{
    #[autometrics]
    pub async fn propose_dlc_channel(
        &self,
        channel_details: ChannelDetails,
        contract_input: ContractInput,
    ) -> Result<()> {
        tracing::info!(channel_id = %hex::encode(channel_details.channel_id), "Sending DLC channel offer");

        spawn_blocking({
            let oracle = self.oracle.clone();
            let sub_channel_manager = self.sub_channel_manager.clone();
            let event_id = contract_input.contract_infos[0].oracles.event_id.clone();
            let dlc_message_handler = self.dlc_message_handler.clone();
            move || {
                let announcement = oracle.get_announcement(&event_id)?;

                let sub_channel_offer = sub_channel_manager.offer_sub_channel(
                    &channel_details.channel_id,
                    &contract_input,
                    &[vec![announcement]],
                )?;

                dlc_message_handler.send_message(
                    channel_details.counterparty.node_id,
                    Message::SubChannel(SubChannelMessage::Offer(sub_channel_offer)),
                );

                Ok(())
            }
        })
        .await?
    }

    #[autometrics]
    pub fn accept_dlc_channel_offer(&self, channel_id: &[u8; 32]) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel offer");

        let (node_id, accept_sub_channel) =
            self.sub_channel_manager.accept_sub_channel(channel_id)?;

        self.dlc_message_handler.send_message(
            node_id,
            Message::SubChannel(SubChannelMessage::Accept(accept_sub_channel)),
        );

        Ok(())
    }

    #[autometrics]
    pub async fn propose_dlc_channel_collaborative_settlement(
        &self,
        channel_id: [u8; 32],
        accept_settlement_amount: u64,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(
            channel_id = %channel_id_hex,
            %accept_settlement_amount,
            "Settling DLC channel collaboratively"
        );

        spawn_blocking({
            let sub_channel_manager = self.sub_channel_manager.clone();
            let dlc_message_handler = self.dlc_message_handler.clone();
            move || {
                let (sub_channel_close_offer, counterparty_pk) = sub_channel_manager
                    .offer_subchannel_close(&channel_id, accept_settlement_amount)?;

                dlc_message_handler.send_message(
                    counterparty_pk,
                    Message::SubChannel(SubChannelMessage::CloseOffer(sub_channel_close_offer)),
                );

                Ok(())
            }
        })
        .await?
    }

    #[autometrics]
    pub fn accept_dlc_channel_collaborative_settlement(&self, channel_id: &[u8; 32]) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative settlement");

        let (sub_channel_close_accept, counterparty_pk) = self
            .sub_channel_manager
            .accept_subchannel_close_offer(channel_id)?;

        self.dlc_message_handler.send_message(
            counterparty_pk,
            Message::SubChannel(SubChannelMessage::CloseAccept(sub_channel_close_accept)),
        );

        Ok(())
    }

    #[autometrics]
    pub fn get_dlc_channel_offer(&self, pubkey: &PublicKey) -> Result<Option<SubChannel>> {
        let dlc_channel = self
            .dlc_manager
            .get_store()
            .get_offered_sub_channels()?
            .into_iter()
            .find(|dlc_channel| dlc_channel.counter_party == *pubkey);

        Ok(dlc_channel)
    }

    #[autometrics]
    pub fn get_temporary_contract_id_by_sub_channel_id(
        &self,
        sub_channel_id: ChannelId,
    ) -> Result<ContractId> {
        let store = self.dlc_manager.get_store();

        let dlc_channel_id = store
            .get_sub_channel(sub_channel_id)?
            .with_context(|| format!("No subchannel found for id {}", sub_channel_id.to_hex()))?
            .get_dlc_channel_id(0)
            .context("No dlc channel with index 0 found")?;

        let contract_id = store
            .get_channel(&dlc_channel_id)?
            .with_context(|| {
                format!(
                    "No dlc channel found for dlc channel id {}",
                    dlc_channel_id.to_hex()
                )
            })?
            .get_contract_id()
            .with_context(|| {
                format!(
                    "No contract id set for dlc channel with id {}",
                    dlc_channel_id.to_hex()
                )
            })?;

        let contract = store.get_contract(&contract_id)?.with_context(|| {
            format!("No contract found for contract id {}", contract_id.to_hex())
        })?;

        Ok(contract.get_temporary_id())
    }

    #[autometrics]
    pub fn get_closed_contract(
        &self,
        temporary_contract_id: ContractId,
    ) -> Result<Option<ClosedContract>> {
        let contract = self
            .dlc_manager
            .get_store()
            .get_contracts()?
            .into_iter()
            .find_map(|contract| match contract {
                Contract::Closed(closed_contract)
                    if closed_contract.temporary_contract_id == temporary_contract_id =>
                {
                    Some(closed_contract)
                }
                _ => None,
            });

        Ok(contract)
    }

    #[autometrics]
    pub fn get_dlc_channel_signed(&self, pubkey: &PublicKey) -> Result<Option<SubChannel>> {
        let matcher = |dlc_channel: &&SubChannel| {
            dlc_channel.counter_party == *pubkey
                && matches!(&dlc_channel.state, SubChannelState::Signed(_))
        };
        let dlc_channel = self.get_dlc_channel(&matcher)?;
        Ok(dlc_channel)
    }

    #[autometrics]
    pub fn get_dlc_channel_close_offer(&self, pubkey: &PublicKey) -> Result<Option<SubChannel>> {
        let matcher = |dlc_channel: &&SubChannel| {
            dlc_channel.counter_party == *pubkey
                && matches!(&dlc_channel.state, SubChannelState::CloseOffered(_))
        };
        let dlc_channel = self.get_dlc_channel(&matcher)?;

        Ok(dlc_channel)
    }

    #[autometrics]
    pub fn list_dlc_channels(&self) -> Result<Vec<SubChannel>> {
        let dlc_channels = self.dlc_manager.get_store().get_sub_channels()?;

        Ok(dlc_channels)
    }

    /// Check if it is safe to close the LN channel collaboratively.
    ///
    /// In general, it is NOT safe to close an LN channel if there still is a DLC channel attached
    /// to it. This is because this can lead to loss of funds.
    #[autometrics]
    pub fn is_safe_to_close_ln_channel_collaboratively(&self, channel_id: &[u8; 32]) -> Result<()> {
        let dlc_channels = self
            .dlc_manager
            .get_store()
            .get_sub_channels()
            .map_err(|e| anyhow!("{e:#}"))?;

        let state = match dlc_channels
            .iter()
            .find(|channel| &channel.channel_id == channel_id)
        {
            Some(channel) => &channel.state,
            // It's safe to close the LN channel if there is no associated DLC channel
            None => return Ok(()),
        };

        tracing::debug!(
            channel_id = %hex::encode(channel_id),
            dlc_channel_state = ?state,
            "Checking if it's safe to close LN channel"
        );

        use SubChannelState::*;
        match state {
            // The channel is in an opening state
            Offered(_) | Accepted(_) | Confirmed(_) | Finalized(_) => bail!("It's unsafe to collaboratively close LN channel when the DLC channel is being opened"),
            // The channel is open,
            Signed(_) => bail!("It's unsafe to collaboratively close LN channel when the DLC channel is open"),
            // The channel is being closed,
            Closing(_) | CloseOffered(_) | CloseAccepted(_) | CloseConfirmed(_) => bail!("It's unsafe to collaboratively close LN channel when the DLC channel is being closed"),
            // It's safe to close the LN channel if there is no associated DLC channel
            OffChainClosed | ClosedPunished(_) | Rejected | CounterOnChainClosed
            | OnChainClosed => {},
        };

        Ok(())
    }

    #[autometrics]
    fn get_dlc_channel(
        &self,
        matcher: impl FnMut(&&SubChannel) -> bool,
    ) -> Result<Option<SubChannel>> {
        let dlc_channels = self.list_dlc_channels()?;
        let dlc_channel = dlc_channels.iter().find(matcher);

        Ok(dlc_channel.cloned())
    }

    #[cfg(test)]
    #[autometrics]
    pub fn process_incoming_messages(&self) -> Result<()> {
        let dlc_message_handler = &self.dlc_message_handler;
        let dlc_manager = &self.dlc_manager;
        let sub_channel_manager = &self.sub_channel_manager;
        let messages = dlc_message_handler.get_and_clear_received_messages();

        for (node_id, msg) in messages {
            match msg {
                Message::OnChain(_) | Message::Channel(_) => {
                    tracing::debug!(from = %node_id, "Processing DLC-manager message");
                    let resp = dlc_manager.on_dlc_message(&msg, node_id)?;

                    if let Some(msg) = resp {
                        tracing::debug!(to = %node_id, "Sending DLC-manager message");
                        dlc_message_handler.send_message(node_id, msg);
                    }
                }
                Message::SubChannel(msg) => {
                    tracing::debug!(
                        from = %node_id,
                        msg = %sub_channel_message_name(&msg),
                        "Processing DLC channel message"
                    );
                    let resp = sub_channel_manager.on_sub_channel_message(&msg, &node_id)?;

                    if let Some(msg) = resp {
                        tracing::debug!(
                            to = %node_id,
                            msg = %sub_channel_message_name(&msg),
                            "Sending DLC channel message"
                        );
                        dlc_message_handler.send_message(node_id, Message::SubChannel(msg));
                    }
                }
            }
        }

        Ok(())
    }
}

#[autometrics]
pub(crate) async fn sub_channel_manager_periodic_check(
    sub_channel_manager: Arc<SubChannelManager>,
    dlc_message_handler: &DlcMessageHandler,
) -> Result<()> {
    let messages = spawn_blocking(move || sub_channel_manager.periodic_check()).await?;

    for (msg, node_id) in messages {
        let msg = Message::SubChannel(msg);
        let msg_name = dlc_message_name(&msg);

        tracing::info!(
            to = %node_id,
            kind = %msg_name,
            "Queuing up DLC channel message tied to pending action"
        );

        dlc_message_handler.send_message(node_id, msg);
    }

    Ok(())
}

pub fn dlc_message_name(msg: &Message) -> String {
    let name = match msg {
        Message::OnChain(OnChainMessage::Offer(_)) => "OnChainOffer",
        Message::OnChain(OnChainMessage::Accept(_)) => "OnChainAccept",
        Message::OnChain(OnChainMessage::Sign(_)) => "OnChainSign",
        Message::Channel(ChannelMessage::Offer(_)) => "ChannelOffer",
        Message::Channel(ChannelMessage::Accept(_)) => "ChannelAccept",
        Message::Channel(ChannelMessage::Sign(_)) => "ChannelSign",
        Message::Channel(ChannelMessage::SettleOffer(_)) => "ChannelSettleOffer",
        Message::Channel(ChannelMessage::SettleAccept(_)) => "ChannelSettleAccept",
        Message::Channel(ChannelMessage::SettleConfirm(_)) => "ChannelSettleConfirm",
        Message::Channel(ChannelMessage::SettleFinalize(_)) => "ChannelSettleFinalize",
        Message::Channel(ChannelMessage::RenewOffer(_)) => "ChannelRenewOffer",
        Message::Channel(ChannelMessage::RenewAccept(_)) => "ChannelRenewAccept",
        Message::Channel(ChannelMessage::RenewConfirm(_)) => "ChannelRenewConfirm",
        Message::Channel(ChannelMessage::RenewFinalize(_)) => "ChannelRenewFinalize",
        Message::Channel(ChannelMessage::RenewRevoke(_)) => "ChannelRenewRevoke",
        Message::Channel(ChannelMessage::CollaborativeCloseOffer(_)) => {
            "ChannelCollaborativeCloseOffer"
        }
        Message::Channel(ChannelMessage::Reject(_)) => "ChannelReject",
        Message::SubChannel(msg) => sub_channel_message_name(msg),
    };

    name.to_string()
}

pub fn sub_channel_message_name(msg: &SubChannelMessage) -> &str {
    use SubChannelMessage::*;
    match msg {
        Offer(_) => "SubChannelOffer",
        Accept(_) => "SubChannelAccept",
        Confirm(_) => "SubChannelConfirm",
        Finalize(_) => "SubChannelFinalize",
        Revoke(_) => "SubChannelRevoke",
        CloseOffer(_) => "SubChannelCloseOffer",
        CloseAccept(_) => "SubChannelCloseAccept",
        CloseConfirm(_) => "SubChannelCloseConfirm",
        CloseFinalize(_) => "SubChannelCloseFinalize",
        Reject(_) => "SubChannelReject",
    }
}
