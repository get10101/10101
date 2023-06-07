use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use bdk::{
    blockchain::{Blockchain, EsploraBlockchain},
    database::BatchDatabase,
    sled::Tree,
    wallet::AddressIndex,
    SyncOptions, TransactionDetails, Wallet,
};
use bitcoin::{Address, Amount, Network};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use xtra::prelude::*;
use xtra_productivity::xtra_productivity;
use xtras::SendInterval;

const SYNC_INTERVAL: Duration = Duration::from_secs(3 * 60);

#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub network: Network,
    pub balance: Amount,
    pub address: Address,
    pub last_updated_at: OffsetDateTime,
    pub transactions: Vec<TransactionDetails>,
}

pub struct WalletActor<B, DB> {
    wallet: Wallet<DB>,
    blockchain_client: B,
    cache: Option<WalletInfo>,
}

impl WalletActor<EsploraBlockchain, Tree> {
    pub fn new(wallet: Wallet<Tree>, blockchain_client: EsploraBlockchain) -> Self {
        Self {
            wallet,
            blockchain_client,
            cache: None,
        }
    }
}

impl<B, DB> WalletActor<B, DB>
where
    B: Blockchain,
    DB: BatchDatabase,
{
    #[tracing::instrument(name = "Sync wallet", skip_all, err)]
    async fn sync_internal(&mut self) -> Result<WalletInfo> {
        let now = Instant::now();
        tracing::trace!(target : "wallet", "Wallet sync started");

        self.wallet
            .sync(&self.blockchain_client, SyncOptions::default())
            .await
            .context("Failed to sync wallet")?;

        let balance =
            tracing::debug_span!("Get wallet balance").in_scope(|| self.wallet.get_balance())?;

        let balance = match self.wallet.network() {
            Network::Bitcoin => balance.get_spendable(),
            _ => balance.get_total(),
        };

        let address = self.wallet.get_address(AddressIndex::LastUnused)?.address;
        let transactions = self.wallet.list_transactions(false)?;

        let wallet_info = WalletInfo {
            network: self.wallet.network(),
            balance: Amount::from_sat(balance),
            address,
            last_updated_at: OffsetDateTime::now_utc(),
            transactions,
        };
        tracing::trace!(target : "wallet", sync_time_sec = %now.elapsed().as_secs(), "Wallet sync done");
        Ok(wallet_info)
    }
}
