use crate::api::Balances;
use crate::api::WalletInfo;
use anyhow::anyhow;
use anyhow::Result;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use dlc_messages::Message;
use ln_dlc_node::node::sub_channel_message_as_str;
use ln_dlc_node::node::DlcManager;
use ln_dlc_node::node::SubChannelManager;
use ln_dlc_node::PeerManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
}

impl Node {
    pub fn get_wallet_info_from_node(&self) -> WalletInfo {
        WalletInfo {
            balances: Balances {
                lightning: self.inner.get_ldk_balance().available,
                on_chain: self
                    .inner
                    .get_on_chain_balance()
                    .expect("balance")
                    .confirmed,
            },
            history: vec![], // TODO: sync history
        }
    }
}

pub(crate) fn process_incoming_messages_internal(
    dlc_message_handler: &DlcMessageHandler,
    dlc_manager: &DlcManager,
    sub_channel_manager: &SubChannelManager,
    peer_manager: &PeerManager,
) -> Result<()> {
    let messages = dlc_message_handler.get_and_clear_received_messages();

    for (node_id, msg) in messages {
        match msg {
            Message::OnChain(_) | Message::Channel(_) => {
                tracing::debug!(from = %node_id, "Processing DLC-manager message");
                let resp = dlc_manager
                    .on_dlc_message(&msg, node_id)
                    .map_err(|e| anyhow!(e.to_string()))?;

                if let Some(msg) = resp {
                    tracing::debug!(to = %node_id, "Sending DLC-manager message");
                    dlc_message_handler.send_message(node_id, msg);
                }
            }
            Message::SubChannel(msg) => {
                tracing::debug!(
                    from = %node_id,
                    msg = %sub_channel_message_as_str(&msg),
                    "Processing sub-channel message"
                );
                let resp = sub_channel_manager
                    .on_sub_channel_message(&msg, &node_id)
                    .map_err(|e| anyhow!(e.to_string()))?;

                if let Some(msg) = resp {
                    tracing::debug!(
                        to = %node_id,
                        msg = %sub_channel_message_as_str(&msg),
                        "Sending sub-channel message"
                    );
                    dlc_message_handler.send_message(node_id, Message::SubChannel(msg));
                }
            }
        }
    }

    // NOTE: According to the docs of `process_events` we shouldn't have to call this since we
    // use `lightning-net-tokio`. But we copied this from `p2pderivatives/ldk-sample`
    if dlc_message_handler.has_pending_messages() {
        peer_manager.process_events();
    }

    Ok(())
}
