use crate::node::Storage;
use crate::node::SubChannelManager;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use anyhow::Result;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::OnChainMessage;
use std::sync::Arc;
use tokio::task::spawn_blocking;

pub(crate) async fn sub_channel_manager_periodic_check<
    D: BdkStorage,
    S: TenTenOneStorage + 'static,
    N: Storage + Sync + Send + 'static,
>(
    sub_channel_manager: Arc<SubChannelManager<D, S, N>>,
) -> Result<()> {
    let messages = spawn_blocking(move || sub_channel_manager.periodic_check()).await?;

    for (msg, node_id) in messages {
        let msg = Message::SubChannel(msg);
        let msg_name = dlc_message_name(&msg);

        tracing::debug!(
            to = %node_id,
            kind = %msg_name,
            "Not sending DLC channel message tied to pending action"
        );
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
        Message::SubChannel(_) => "SubChannelMessage",
    };

    name.to_string()
}
