use crate::fee_rate_estimator::FeeRateEstimator;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use autometrics::autometrics;
use bdk::blockchain::Blockchain;
use bdk::blockchain::EsploraBlockchain;
use bdk::blockchain::GetHeight;
use bdk::database::BatchDatabase;
use bdk::wallet::AddressIndex;
use bdk::SignOptions;
use bdk::SyncOptions;
use bdk::TransactionDetails;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::BlockHash;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::Txid;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use parking_lot::Mutex;
use parking_lot::MutexGuard;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Wallet<D>
where
    D: BatchDatabase,
{
    // A BDK blockchain used for wallet sync.
    pub(crate) blockchain: Arc<EsploraBlockchain>,
    // A BDK on-chain wallet.
    inner: Mutex<bdk::Wallet<D>>,
    settings: RwLock<WalletSettings>,
    fee_rate_estimator: Arc<FeeRateEstimator>,
}

#[derive(Clone, Debug, Default)]
pub struct WalletSettings {
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,
}

impl<D> Wallet<D>
where
    D: BatchDatabase,
{
    pub(crate) fn new(
        blockchain: EsploraBlockchain,
        wallet: bdk::Wallet<D>,
        fee_rate_estimator: Arc<FeeRateEstimator>,
    ) -> Self {
        let inner = Mutex::new(wallet);
        let settings = RwLock::new(WalletSettings::default());

        Self {
            blockchain: Arc::new(blockchain),
            inner,
            settings,
            fee_rate_estimator,
        }
    }

    fn bdk_lock(&self) -> MutexGuard<bdk::Wallet<D>> {
        self.inner.lock()
    }

    pub async fn update_settings(&self, settings: WalletSettings) {
        tracing::info!(?settings, "Updating wallet settings");
        *self.settings.write().await = settings;
    }

    pub async fn settings(&self) -> WalletSettings {
        self.settings.read().await.clone()
    }

    /// Update the internal BDK wallet database with the blockchain.
    pub async fn sync(&self) -> Result<()> {
        let wallet_lock = self.bdk_lock();

        wallet_lock.sync(&self.blockchain, SyncOptions::default())?;

        Ok(())
    }

    #[autometrics]
    pub(crate) async fn create_funding_transaction(
        &self,
        output_script: Script,
        value_sats: u64,
        confirmation_target: ConfirmationTarget,
    ) -> Result<Transaction, Error> {
        let locked_wallet = self.bdk_lock();
        let mut tx_builder = locked_wallet.build_tx();

        let fee_rate = self.fee_rate_estimator.get(confirmation_target);
        tx_builder
            .add_recipient(output_script, value_sats)
            .fee_rate(fee_rate)
            .enable_rbf();

        let mut psbt = match tx_builder.finish() {
            Ok((psbt, _)) => {
                tracing::trace!("Created funding PSBT: {:?}", psbt);
                psbt
            }
            Err(err) => {
                tracing::error!("Failed to create funding transaction: {}", err);
                return Err(err.into());
            }
        };

        match locked_wallet.sign(&mut psbt, SignOptions::default()) {
            Ok(finalized) => {
                if !finalized {
                    bail!("Onchain transaction failed");
                }
            }
            Err(err) => {
                tracing::error!("Failed to create funding transaction: {}", err);
                return Err(err.into());
            }
        }

        Ok(psbt.extract_tx())
    }

    #[autometrics]
    pub(crate) fn get_new_address(&self) -> Result<bitcoin::Address, Error> {
        Ok(self.bdk_lock().get_address(AddressIndex::New)?.address)
    }

    #[autometrics]
    pub(crate) fn get_last_unused_address(&self) -> Result<bitcoin::Address, Error> {
        Ok(self
            .bdk_lock()
            .get_address(AddressIndex::LastUnused)?
            .address)
    }

    #[autometrics]
    pub(crate) fn get_balance(&self) -> Result<bdk::Balance, Error> {
        Ok(self.bdk_lock().get_balance()?)
    }

    /// Send funds to the given address.
    ///
    /// If `amount_msat_or_drain` is `None` the wallet will be drained, i.e., all available funds
    /// will be spent.
    #[allow(dead_code)]
    #[autometrics]
    pub(crate) fn send_to_address(
        &self,
        address: &bitcoin::Address,
        amount_msat_or_drain: Option<u64>,
    ) -> Result<Txid> {
        let fee_rate = self.fee_rate_estimator.get(ConfirmationTarget::Normal);

        let tx = {
            let locked_wallet = self.bdk_lock();
            let mut tx_builder = locked_wallet.build_tx();

            if let Some(amount_sats) = amount_msat_or_drain {
                tx_builder
                    .add_recipient(address.script_pubkey(), amount_sats)
                    .fee_rate(fee_rate)
                    .enable_rbf();
            } else {
                tx_builder
                    .drain_wallet()
                    .drain_to(address.script_pubkey())
                    .fee_rate(fee_rate)
                    .enable_rbf();
            }

            let mut psbt = match tx_builder.finish() {
                Ok((psbt, _)) => {
                    tracing::trace!("Created PSBT: {:?}", psbt);
                    psbt
                }
                Err(err) => {
                    bail!(err)
                }
            };

            match locked_wallet.sign(&mut psbt, SignOptions::default()) {
                Ok(finalized) => {
                    if !finalized {
                        bail!("On chain creation failed");
                    }
                }
                Err(err) => {
                    bail!(err)
                }
            }
            psbt.extract_tx()
        };

        self.broadcast_transaction(&tx);

        let txid = tx.txid();

        if let Some(amount_sats) = amount_msat_or_drain {
            tracing::info!(
                "Created new transaction {} sending {}sats on-chain to address {}",
                txid,
                amount_sats,
                address
            );
        } else {
            tracing::info!(
                "Created new transaction {} sending all available on-chain funds to address {}",
                txid,
                address
            );
        }

        Ok(txid)
    }

    #[autometrics]
    pub fn tip(&self) -> Result<(u32, BlockHash)> {
        let height = self.blockchain.get_height()?;
        let hash = self.blockchain.get_tip_hash()?;

        Ok((height, hash))
    }

    #[autometrics]
    pub async fn on_chain_transaction_list(&self) -> Result<Vec<TransactionDetails>> {
        let wallet_lock = self.bdk_lock();
        wallet_lock
            .list_transactions(false)
            .context("Failed to list on chain transactions")
    }
}

impl<D> BroadcasterInterface for Wallet<D>
where
    D: BatchDatabase,
{
    fn broadcast_transaction(&self, tx: &Transaction) {
        let txid = tx.txid();

        tracing::info!(%txid, raw_tx = %serialize_hex(&tx), "Broadcasting transaction");

        if let Err(err) = self.blockchain.broadcast(tx) {
            tracing::error!("Failed to broadcast transaction: {err:#}");
        }
    }
}
