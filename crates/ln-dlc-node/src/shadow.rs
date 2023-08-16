use crate::channel::Channel;
use crate::channel::ChannelState;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::ChannelManager;
use crate::node::Storage;
use anyhow::Result;
use bdk::TransactionDetails;
use dlc_manager::subchannel::LNChannelManager;
use std::sync::Arc;

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

        for transaction in transactions.iter() {
            let txid = transaction.txid();
            match self.ln_dlc_wallet.inner().get_transaction(&txid) {
                Ok(Some(TransactionDetails { fee: Some(fee), .. })) => {
                    self.storage.upsert_transaction(transaction.with_fee(fee))?;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(%txid, "Failed to get transaction details: {e:#}");
                }
            };
        }
        Ok(())
    }
}
