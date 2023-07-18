use crate::channel::Channel;
use crate::channel::ChannelState;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::ChannelManager;
use crate::node::Storage;
use anyhow::Result;
use dlc_manager::subchannel::LNChannelManager;
use std::sync::Arc;
use time::OffsetDateTime;

pub struct Shadow<S> {
    storage: Arc<S>,
    ln_dlc_wallet: Arc<LnDlcWallet>,
    channel_manager: Arc<ChannelManager>,
}

impl<S> Shadow<S>
where
    S: Storage + Send + Sync + 'static,
{
    pub fn new(
        storage: Arc<S>,
        ln_dlc_wallet: Arc<LnDlcWallet>,
        channel_manager: Arc<ChannelManager>,
    ) -> Self {
        Shadow {
            storage,
            ln_dlc_wallet,
            channel_manager,
        }
    }

    pub fn sync_channels(&self) -> Result<()> {
        let channels = self.storage.all_non_pending_channels()?;
        tracing::debug!("Syncing {} shadow channels", channels.len());

        for mut channel in channels
            .into_iter()
            .filter(|c| c.channel_state == ChannelState::Open)
        {
            if let Some(channel_id) = &channel.channel_id {
                let channel_details = self.channel_manager.get_channel_details(channel_id);
                channel = Channel::update_liquidity(&channel, &channel_details)?;
                self.storage.upsert_channel(channel.clone())?;
            }
        }
        Ok(())
    }

    pub fn sync_transactions(&self) -> Result<()> {
        let transactions = self.storage.all_transactions_without_fees()?;
        tracing::debug!("Syncing {} shadow transactions", transactions.len());

        for mut transaction in transactions.into_iter() {
            let transaction_details = self
                .ln_dlc_wallet
                .inner()
                .get_transaction(&transaction.txid)?;

            transaction.fee = transaction_details
                .map(|d| d.fee.unwrap_or_default())
                .unwrap_or_default();
            transaction.updated_at = OffsetDateTime::now_utc();

            self.storage.upsert_transaction(transaction)?;
        }
        Ok(())
    }
}
