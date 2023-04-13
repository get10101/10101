use crate::node::Node;
use crate::node::NodeInfo;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use lightning::ln::channelmanager::ChannelDetails;

impl<P> Node<P> {
    /// Initiates the open private channel protocol.
    ///
    /// Returns a temporary channel ID as a 32-byte long array.
    pub fn initiate_open_channel(
        &self,
        peer: NodeInfo,
        channel_amount_sat: u64,
        initial_send_amount_sats: u64,
    ) -> Result<[u8; 32]> {
        let mut user_config = self.user_config;
        user_config.channel_handshake_config.announced_channel = false;

        let temp_channel_id = self
            .channel_manager
            .create_channel(
                peer.pubkey,
                channel_amount_sat,
                initial_send_amount_sats * 1000,
                0,
                Some(user_config),
            )
            .map_err(|e| anyhow!("{e:?}"))
            .with_context(|| format!("Could not create channel with {peer}"))?;

        tracing::info!(
            %peer,
            temp_channel_id = %hex::encode(temp_channel_id),
            "Started channel creation"
        );

        Ok(temp_channel_id)
    }

    pub fn list_usable_channels(&self) -> Vec<ChannelDetails> {
        self.channel_manager.list_usable_channels()
    }

    pub fn list_channels(&self) -> Vec<ChannelDetails> {
        self.channel_manager.list_channels()
    }

    pub fn list_peers(&self) -> Vec<PublicKey> {
        self.peer_manager.get_peer_node_ids()
    }
}
