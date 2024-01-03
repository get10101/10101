use crate::node::Node;
use crate::node::Storage;
use crate::storage::TenTenOneStorage;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Txid;
use lightning::chain::channelmonitor::Balance;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::ln::ChannelId;
use lightning::util::persist::read_channel_monitors;

impl<S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static> Node<S, N> {
    /// Initiates the open private channel protocol.
    ///
    /// Returns a temporary channel ID as a 32-byte long array.
    pub fn initiate_open_channel(
        &self,
        counterparty_node_id: PublicKey,
        channel_amount_sat: u64,
        initial_send_amount_sats: u64,
        public_channel: bool,
    ) -> Result<ChannelId> {
        let mut ldk_config = *self.ldk_config.read();
        ldk_config.channel_handshake_config.announced_channel = public_channel;

        let temp_channel_id = self
            .channel_manager
            .create_channel(
                counterparty_node_id,
                channel_amount_sat,
                initial_send_amount_sats * 1000,
                0,
                Some(ldk_config),
            )
            .map_err(|e| anyhow!("{e:?}"))
            .with_context(|| format!("Could not create channel with {counterparty_node_id}"))?;

        tracing::info!(
            %counterparty_node_id,
            temp_channel_id = %hex::encode(temp_channel_id.0),
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

    pub fn get_channel_balances(&self, txid: Txid) -> Result<Option<Vec<Balance>>> {
        let vec = read_channel_monitors(
            self.ln_storage.clone(),
            self.keys_manager.clone(),
            self.keys_manager.clone(),
        )?;
        let channel_monitor = vec.iter().find_map(|(_, monitor)| {
            if monitor.get_original_funding_txo().0.txid == txid {
                Some(monitor.get_claimable_balances())
            } else {
                None
            }
        });
        Ok(channel_monitor)
    }

    pub fn list_peers(&self) -> Vec<PublicKey> {
        self.peer_manager
            .get_peer_node_ids()
            .into_iter()
            .map(|(peer, _)| peer)
            .collect()
    }

    pub fn close_channel(&self, channel_id: ChannelId, force_close: bool) -> Result<()> {
        let channel_id_str = hex::encode(channel_id.0);

        let channels = self.channel_manager.list_channels();
        let channel = channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
            .with_context(|| format!("Cannot close non-existent channel {channel_id_str}"))?;

        if force_close {
            self.force_close_channel(channel)?;
        } else {
            self.collab_close_channel(channel)?;
        }

        Ok(())
    }

    fn collab_close_channel(&self, channel: &ChannelDetails) -> Result<()> {
        let channel_id = channel.channel_id;
        let channel_id_str = hex::encode(channel_id.0);
        let peer = channel.counterparty.node_id;

        tracing::info!(channel_id = %hex::encode(channel_id.0), %peer, "Collaboratively closing channel");

        self.is_safe_to_close_ln_channel_collaboratively(&channel_id)
            .with_context(|| {
                format!("Could not collaboratively close LN channel {channel_id_str}: must close DLC channel first")
            })?;

        self.channel_manager
            .close_channel(&channel_id, &peer)
            .map_err(|e| {
                anyhow!("Could not collaboratively close channel {channel_id_str}: {e:?}")
            })?;

        Ok(())
    }

    pub(crate) fn force_close_channel(&self, channel: &ChannelDetails) -> Result<()> {
        let channel_id = channel.channel_id;
        let channel_id_str = hex::encode(channel_id.0);
        let peer = channel.counterparty.node_id;

        let has_dlc_channel = self
            .list_sub_channels()?
            .iter()
            .any(|channel| channel.channel_id == channel_id);

        if has_dlc_channel {
            tracing::info!(
                channel_id = %hex::encode(channel_id.0),
                %peer,
                "Initiating force-closure of LN-DLC channel"
            );
            self.sub_channel_manager
                .force_close_sub_channel(&channel_id)
                .map_err(|e| anyhow!("Failed to initiate force-close: {e:#}"))?
        } else {
            tracing::info!(channel_id = %hex::encode(channel_id.0), %peer, "Force-closing LN channel");
            self.channel_manager
                .force_close_broadcasting_latest_txn(&channel_id, &peer)
                .map_err(|e| anyhow!("Could not force-close channel {channel_id_str}: {e:?}"))?;
        };

        Ok(())
    }

    pub fn sign_message(&self, data: String) -> Result<String> {
        let secret = self.keys_manager.get_node_secret_key();
        let signature = lightning::util::message_signing::sign(data.as_bytes(), &secret)?;
        Ok(signature)
    }
}
