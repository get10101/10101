use crate::node::Node;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::Recipient;
use lightning::ln::channelmanager::ChannelDetails;

impl<P> Node<P> {
    /// Initiates the open private channel protocol.
    ///
    /// Returns a temporary channel ID as a 32-byte long array.
    pub fn initiate_open_channel(
        &self,
        counterparty_node_id: PublicKey,
        channel_amount_sat: u64,
        initial_send_amount_sats: u64,
        public_channel: bool,
    ) -> Result<[u8; 32]> {
        let mut user_config = self.user_config;
        user_config.channel_handshake_config.announced_channel = public_channel;

        let temp_channel_id = self
            .channel_manager
            .create_channel(
                counterparty_node_id,
                channel_amount_sat,
                initial_send_amount_sats * 1000,
                0,
                Some(user_config),
            )
            .map_err(|e| anyhow!("{e:?}"))
            .with_context(|| format!("Could not create channel with {counterparty_node_id}"))?;

        tracing::info!(
            %counterparty_node_id,
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

    pub fn close_channel(&self, channel_id: [u8; 32], force_close: bool) -> Result<()> {
        let channel_manager = self.channel_manager.clone();
        let all_channels = channel_manager.list_channels();
        let channels_to_close = all_channels
            .iter()
            .find(|channel| channel.channel_id == channel_id);

        match channels_to_close {
            Some(cd) => {
                if force_close {
                    tracing::debug!(
                        "Force closing channel {} with peer {} ",
                        hex::encode(cd.channel_id),
                        cd.counterparty.node_id
                    );
                    channel_manager
                        .force_close_broadcasting_latest_txn(
                            &cd.channel_id,
                            &cd.counterparty.node_id,
                        )
                        .map_err(|e| anyhow!("Could not force close channel {e:?}"))
                } else {
                    tracing::info!(
                        "Collaboratively closing channel {} with peer {} ",
                        hex::encode(cd.channel_id),
                        cd.counterparty.node_id
                    );
                    channel_manager
                        .close_channel(&cd.channel_id, &cd.counterparty.node_id)
                        .map_err(|e| anyhow!("Could not collaboratively close channel {e:?}"))
                }
            }
            None => {
                bail!("No channel found with ID {}", hex::encode(channel_id))
            }
        }
    }

    pub fn sign_message(&self, data: String) -> Result<String> {
        let secret = self
            .keys_manager
            .get_node_secret(Recipient::Node)
            .map_err(|_| anyhow!("Could not get node's secret"))?;
        let signature = lightning::util::message_signing::sign(data.as_bytes(), &secret)?;
        Ok(signature)
    }
}
