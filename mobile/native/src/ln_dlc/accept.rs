use crate::config;
use crate::ln_dlc::runtime;
use crate::ln_dlc::Node;
use anyhow::Result;
use std::time::Duration;

// TODO: Set to 5 seconds for test purposes; this might not be feasible once in production
const PROCESS_TRADE_REQUESTS_INTERVAL: Duration = Duration::from_secs(5);

impl Node {
    /// Starts a task to accept positions in an `PROCESS_TRADE_REQUESTS_INTERVAL`
    pub fn start_accept_offers_task(&self) -> Result<()> {
        runtime()?.spawn({
            let node = self.clone();
            async move {
                loop {
                    node.accept_position_offers().await;
                    node.accept_position_close_offers().await;
                    tokio::time::sleep(PROCESS_TRADE_REQUESTS_INTERVAL).await;
                }
            }
        });
        Ok(())
    }

    async fn accept_position_offers(&self) {
        let peer = config::get_coordinator_info();
        let pk = peer.pubkey;

        tracing::trace!(%peer, "Checking for DLC offers");

        let sub_channel = match self.inner.get_sub_channel_offer(&pk) {
            Ok(Some(sub_channel)) => sub_channel,
            Ok(None) => {
                tracing::trace!(%peer, "No DLC channel offers found");
                return;
            }
            Err(e) => {
                tracing::error!(peer = %pk.to_string(), "Unable to retrieve DLC channel offer: {e:#}");
                return;
            }
        };

        tracing::debug!(%peer, "Found DLC channel offer");
        let channel_id = sub_channel.channel_id;
        tracing::info!(%peer, channel_id = %hex::encode(channel_id), "Accepting DLC channel offer");

        if let Err(e) = self.inner.accept_dlc_channel_offer(&channel_id) {
            tracing::error!(channel_id = %hex::encode(channel_id), "Failed to accept subchannel: {e:#}");
        };
    }

    async fn accept_position_close_offers(&self) {
        let peer = config::get_coordinator_info();
        let pk = peer.pubkey;

        tracing::trace!(%peer, "Checking for DLC close offers");

        let sub_channel = match self.inner.get_sub_channel_close_offer(&pk) {
            Ok(Some(sub_channel)) => sub_channel,
            Ok(None) => {
                tracing::trace!(%peer, "No DLC channel close offers found");
                return;
            }
            Err(e) => {
                tracing::error!(peer = %pk.to_string(), "Unable to retrieve DLC channel close offer: {e:#}");
                return;
            }
        };

        tracing::debug!(%peer, "Found DLC channel close offer");
        let channel_id = sub_channel.channel_id;
        tracing::info!(%peer, channel_id = %hex::encode(channel_id), "Accepting DLC channel close offer");

        if let Err(e) = self
            .inner
            .accept_dlc_channel_collaborative_settlement(&channel_id)
        {
            tracing::error!(channel_id = %hex::encode(channel_id), "Failed to accept close subchannel: {e:#}");
        };
    }
}
