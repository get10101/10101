use crate::ln_dlc::node::Node;
use anyhow::Result;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannelState;
use ln_dlc_node::node::rust_dlc_manager::channel::Channel;
use ln_dlc_node::node::rust_dlc_manager::Storage;

impl Node {
    /// Checks and recovers from a potential stuck rollover.
    pub async fn recover_rollover(&self) -> Result<()> {
        tracing::debug!("Checking if dlc channel got stuck in rollover.");
        let channels = self.inner.channel_manager.list_channels();
        let channel_details = match channels.first() {
            Some(channel_details) => channel_details,
            None => {
                tracing::debug!("No channel found. All good!");
                return Ok(());
            }
        };

        let dlc_channels = self.inner.dlc_manager.get_store().get_sub_channels()?;
        let dlc_channel = dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.channel_id == channel_details.channel_id);

        let dlc_channel = match dlc_channel {
            Some(dlc_channel) => dlc_channel,
            None => {
                tracing::debug!("No dlc channel found. All good!");
                return Ok(());
            }
        };

        let dlc_channel_id = match dlc_channel.get_dlc_channel_id(0) {
            Some(dlc_channel_id) => dlc_channel_id,
            None => {
                tracing::warn!(channel_id=%hex::encode(dlc_channel.channel_id), "Couldn't get a dlc channel id for a dlc channel");
                return Ok(());
            }
        };

        let channel = self
            .inner
            .dlc_manager
            .get_store()
            .get_channel(&dlc_channel_id)?;

        let signed_channel = match channel {
            Some(Channel::Signed(signed_channel)) => signed_channel,
            Some(channel) => {
                tracing::warn!(dlc_channel_id=%hex::encode(dlc_channel_id), "Found channel in unexpected state. Expected: Signed, Found: {channel:?}");
                return Ok(());
            }
            None => {
                tracing::warn!(dlc_channel_id=%hex::encode(dlc_channel_id), "Couldn't find channel");
                return Ok(());
            }
        };

        match signed_channel.state {
            SignedChannelState::RenewOffered { .. }
            | SignedChannelState::RenewAccepted { .. }
            | SignedChannelState::RenewConfirmed { .. }
            | SignedChannelState::RenewFinalized { .. } => {
                let state = ln_dlc_node::node::signed_channel_state_name(&signed_channel);
                tracing::warn!(state, "Found signed channel contract in an intermediate state. Rolling back, expecting coordinator to retry rollover!");
                self.inner.rollback_channel(&signed_channel)?;
            }
            _ => {
                tracing::debug!(signed_channel_state=%signed_channel.state, "Channel is not in an intermediate rollover state. All good.")
            }
        }
        Ok(())
    }
}
