use anyhow::bail;
use anyhow::Context as _;
use anyhow::Result;
use async_trait::async_trait;
use bdk::blockchain::EsploraBlockchain;
use bdk::wallet::AddressIndex;
use bdk::Balance;
use bdk::FeeRate;
use bdk::SignOptions;
use bdk::SyncOptions;
use bdk::TransactionDetails;
use bitcoin::Address;
use bitcoin::Script;
use bitcoin::Transaction;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use time::OffsetDateTime;
use xtra::Mailbox;

/// An actor that manages the [`bdk::Wallet`] resource.
///
/// It allows us to use the wallet whilst the inevitably expensive on-chain sync is happening in the
/// background.
///
/// We would like to use the async version of [`EsploraBlockchain`], but
/// https://github.com/bitcoindevkit/bdk/issues/165 is still an issue.
pub struct BdkActor {
    wallet: bdk::Wallet<bdk::sled::Tree>,
    blockchain_client: EsploraBlockchain,
    sync_interval: Arc<RwLock<Duration>>,
}

#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub balance: Balance,
    pub transactions: Vec<TransactionDetails>,
    pub last_updated_at: OffsetDateTime,
}

/// Message to trigger an on-chain sync.
#[derive(Clone, Copy)]
pub struct Sync;

/// Message to get new on-chain address.
#[derive(Clone, Copy)]
pub struct GetNewAddress;

/// Message to get last unused on-chain address.
#[derive(Clone, Copy)]
pub struct GetLastUnusedAddress;

/// Message to get current on-chain balance.
#[derive(Clone, Copy)]
pub struct GetBalance;

/// Message to get current on-chain balance.
#[derive(Clone, Copy)]
pub struct GetHistory;

/// Message to get current on-chain balance.
#[derive(Clone)]
pub struct BuildAndSignTx {
    pub script_pubkey: Script,
    pub amount_sats_or_drain: Option<u64>,
    pub fee_rate: FeeRate,
}

/// Message to set the on-chain sync interval.
pub struct UpdateSyncInterval(pub Duration);

impl BdkActor {
    pub fn new(
        wallet: bdk::Wallet<bdk::sled::Tree>,
        blockchain_client: EsploraBlockchain,
        sync_interval: Duration,
    ) -> Self {
        Self {
            wallet,
            blockchain_client,
            sync_interval: Arc::new(RwLock::new(sync_interval)),
        }
    }
}

impl BdkActor {
    #[tracing::instrument(name = "On-chain sync", skip_all, err)]
    async fn sync(&mut self) -> Result<WalletInfo> {
        let now = Instant::now();
        tracing::debug!("On-chain sync started");

        self.wallet
            .sync(&self.blockchain_client, SyncOptions::default())
            .context("Failed to sync on-chain wallet")?;

        let balance = self.wallet.get_balance()?;
        let transactions = self.wallet.list_transactions(false)?;

        let wallet_info = WalletInfo {
            balance,
            last_updated_at: OffsetDateTime::now_utc(),
            transactions,
        };

        tracing::trace!(sync_time_ms = %now.elapsed().as_millis(), "On-chain sync done");

        Ok(wallet_info)
    }
}

#[async_trait]
impl xtra::Actor for BdkActor {
    type Stop = ();

    async fn started(&mut self, mailbox: &mut Mailbox<Self>) -> Result<(), Self::Stop> {
        tokio::spawn({
            let this = mailbox.address();
            let sync_interval = self.sync_interval.clone();
            async move {
                let sync_interval = *sync_interval.read().expect("RwLock to not be poisoned");
                while this.send(Sync).await.is_ok() {
                    tokio::time::sleep(sync_interval).await;
                }

                tracing::warn!("On-chain sync stopped because actor shut down");
            }
        });

        Ok(())
    }

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl xtra::Handler<Sync> for BdkActor {
    type Return = Result<WalletInfo>;

    async fn handle(&mut self, _: Sync, _: &mut xtra::Context<Self>) -> Self::Return {
        self.sync().await
    }
}

#[async_trait]
impl xtra::Handler<GetNewAddress> for BdkActor {
    type Return = Result<Address>;

    async fn handle(&mut self, _: GetNewAddress, _: &mut xtra::Context<Self>) -> Self::Return {
        Ok(self.wallet.get_address(AddressIndex::New)?.address)
    }
}

#[async_trait]
impl xtra::Handler<GetLastUnusedAddress> for BdkActor {
    type Return = Result<Address>;

    async fn handle(
        &mut self,
        _: GetLastUnusedAddress,
        _: &mut xtra::Context<Self>,
    ) -> Self::Return {
        Ok(self.wallet.get_address(AddressIndex::LastUnused)?.address)
    }
}

#[async_trait]
impl xtra::Handler<GetBalance> for BdkActor {
    type Return = Result<Balance>;

    async fn handle(&mut self, _: GetBalance, _: &mut xtra::Context<Self>) -> Self::Return {
        self.wallet.get_balance().context("Failed to get balance")
    }
}

#[async_trait]
impl xtra::Handler<GetHistory> for BdkActor {
    type Return = Result<Vec<TransactionDetails>>;

    async fn handle(&mut self, _: GetHistory, _: &mut xtra::Context<Self>) -> Self::Return {
        self.wallet
            .list_transactions(false)
            .context("Failed to get transactions")
    }
}

#[async_trait]
impl xtra::Handler<BuildAndSignTx> for BdkActor {
    type Return = Result<Transaction>;

    async fn handle(&mut self, msg: BuildAndSignTx, _: &mut xtra::Context<Self>) -> Self::Return {
        let BuildAndSignTx {
            script_pubkey,
            amount_sats_or_drain,
            fee_rate,
        } = msg;

        let tx = {
            let mut tx_builder = self.wallet.build_tx();
            if let Some(amount_sats) = amount_sats_or_drain {
                tx_builder
                    .add_recipient(script_pubkey, amount_sats)
                    .fee_rate(fee_rate)
                    .enable_rbf();
            } else {
                tx_builder
                    .drain_wallet()
                    .drain_to(script_pubkey)
                    .fee_rate(fee_rate)
                    .enable_rbf();
            }

            let (mut psbt, _) = tx_builder.finish()?;

            if !self.wallet.sign(&mut psbt, SignOptions::default())? {
                bail!("Failed to finalize PSBT");
            }

            psbt.extract_tx()
        };

        let txid = tx.txid();
        if let Some(amount_sats) = amount_sats_or_drain {
            tracing::info!(
                %txid,
                %amount_sats,
                "Built new transaction",
            );
        } else {
            tracing::info!(
                %txid,
                "Built new transaction draining on-chain funds",
            );
        }

        Ok(tx)
    }
}

#[async_trait]
impl xtra::Handler<UpdateSyncInterval> for BdkActor {
    type Return = ();

    async fn handle(
        &mut self,
        msg: UpdateSyncInterval,
        _: &mut xtra::Context<Self>,
    ) -> Self::Return {
        *self
            .sync_interval
            .write()
            .expect("RwLock to not be poisoned") = msg.0;
    }
}
