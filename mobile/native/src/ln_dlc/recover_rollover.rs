use crate::ln_dlc::node::Node;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannelState;
use ln_dlc_node::node::rust_dlc_manager::channel::Channel;
use ln_dlc_node::node::rust_dlc_manager::Storage;

impl Node {
    /// Checks and recovers from a potential stuck rollover.
    pub async fn recover_rollover(&self) -> Result<()> {
        tracing::debug!("Checking if DLC channel got stuck during rollover");

        let channels = self.inner.channel_manager.list_channels();
        let channel_details = match channels.first() {
            Some(channel_details) => channel_details,
            None => {
                tracing::debug!("No need to recover rollover: no LN channel found");
                return Ok(());
            }
        };

        let subchannels = self.inner.dlc_manager.get_store().get_sub_channels()?;
        let subchannel = match subchannels
            .iter()
            .find(|dlc_channel| dlc_channel.channel_id == channel_details.channel_id)
        {
            Some(subchannel) => subchannel,
            None => {
                tracing::debug!("No need to recover rollover: no subchannel found");
                return Ok(());
            }
        };

        let channel_id_hex = subchannel.channel_id.to_hex();

        let dlc_channel_id = match subchannel.get_dlc_channel_id(0) {
            Some(dlc_channel_id) => dlc_channel_id,
            None => {
                tracing::debug!(
                    channel_id=%channel_id_hex,
                    "Cannot consider subchannel for rollover recovery without DLC channel ID"
                );
                return Ok(());
            }
        };

        let dlc_channel_id_hex = dlc_channel_id.to_hex();

        let dlc_channel = self
            .inner
            .dlc_manager
            .get_store()
            .get_channel(&dlc_channel_id)?;

        let signed_channel = match dlc_channel {
            Some(Channel::Signed(signed_channel)) => signed_channel,
            Some(_) => {
                tracing::debug!(
                    channel_id=%channel_id_hex,
                    "No need to recover rollover: DLC channel not signed"
                );
                return Ok(());
            }
            None => {
                tracing::warn!(
                    channel_id=%channel_id_hex,
                    dlc_channel_id=%dlc_channel_id_hex,
                    "Expected DLC channel associated with subchannel not found"
                );
                return Ok(());
            }
        };

        match signed_channel.state {
            SignedChannelState::RenewOffered { .. }
            | SignedChannelState::RenewAccepted { .. }
            | SignedChannelState::RenewConfirmed { .. }
            | SignedChannelState::RenewFinalized { .. } => {
                let state = ln_dlc_node::node::signed_channel_state_name(&signed_channel);

                tracing::warn!(
                    state,
                    "Found signed DLC channel in an intermediate rollover state. \
                     Rolling back and expecting coordinator to retry rollover"
                );

                self.inner.rollback_channel(&signed_channel)?;
            }
            _ => {
                tracing::debug!(
                    signed_channel_state=%signed_channel.state,
                    "No need to recover rollover: DLC channel is not in an \
                     intermediate rollover state"
                )
            }
        }

        Ok(())
    }
}
