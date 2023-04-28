use bitcoin::bech32::u5;
use bitcoin::secp256k1::ecdh::SharedSecret;
use bitcoin::secp256k1::ecdsa::RecoverableSignature;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Scalar;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::Signing;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::TxOut;
use bitcoin::Txid;

use crate::ldk_node_error::Error;
use crate::TracingLogger;
use bdk::blockchain::Blockchain;
use bdk::blockchain::EsploraBlockchain;
use bdk::database::BatchDatabase;
use bdk::wallet::AddressIndex;
use bdk::FeeRate;
use bdk::SignOptions;
use bdk::SyncOptions;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::chain::chaininterface::FEERATE_FLOOR_SATS_PER_KW;
use lightning::chain::keysinterface::InMemorySigner;
use lightning::chain::keysinterface::KeyMaterial;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::ln::msgs::DecodeError;
use lightning::ln::script::ShutdownScript;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;

pub struct Wallet<D>
where
    D: BatchDatabase,
{
    // A BDK blockchain used for wallet sync.
    blockchain: EsploraBlockchain,
    // A BDK on-chain wallet.
    inner: Mutex<bdk::Wallet<D>>,
    // A cache storing the most recently retrieved fee rate estimations.
    fee_rate_cache: RwLock<HashMap<ConfirmationTarget, FeeRate>>,
    runtime: Arc<RwLock<Option<tokio::runtime::Runtime>>>,
    sync_lock: (Mutex<()>, Condvar),
    logger: Arc<TracingLogger>,
}

impl<D> Wallet<D>
where
    D: BatchDatabase,
{
    pub(crate) fn new(
        blockchain: EsploraBlockchain,
        wallet: bdk::Wallet<D>,
        runtime: Arc<RwLock<Option<tokio::runtime::Runtime>>>,
        logger: Arc<FilesystemLogger>,
    ) -> Self {
        let inner = Mutex::new(wallet);
        let fee_rate_cache = RwLock::new(HashMap::new());
        let sync_lock = (Mutex::new(()), Condvar::new());
        Self {
            blockchain,
            inner,
            fee_rate_cache,
            runtime,
            sync_lock,
            logger,
        }
    }

    pub(crate) async fn sync(&self) -> Result<(), Error> {
        let (lock, cvar) = &self.sync_lock;

        let guard = match lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::info!(self.logger, "Sync in progress, skipping.");
                let guard = cvar.wait(lock.lock().unwrap());
                drop(guard);
                cvar.notify_all();
                return Ok(());
            }
        };

        match self.update_fee_estimates().await {
            Ok(()) => (),
            Err(e) => {
                tracing::error!(self.logger, "Fee estimation error: {}", e);
                return Err(e);
            }
        }

        let sync_options = SyncOptions { progress: None };
        let wallet_lock = self.inner.lock().unwrap();
        let res = match wallet_lock.sync(&self.blockchain, sync_options).await {
            Ok(()) => Ok(()),
            Err(e) => match e {
                bdk::Error::Esplora(ref be) => match **be {
                    bdk::blockchain::esplora::EsploraError::Reqwest(_) => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        tracing::error!(
                            self.logger,
                            "Sync failed due to HTTP connection error, retrying: {}",
                            e
                        );
                        let sync_options = SyncOptions { progress: None };
                        wallet_lock
                            .sync(&self.blockchain, sync_options)
                            .await
                            .map_err(|e| From::from(e))
                    }
                    _ => {
                        tracing::error!(self.logger, "Sync failed due to Esplora error: {}", e);
                        Err(From::from(e))
                    }
                },
                _ => {
                    tracing::error!(self.logger, "Wallet sync error: {}", e);
                    Err(From::from(e))
                }
            },
        };

        drop(guard);
        cvar.notify_all();
        res
    }

    pub(crate) async fn update_fee_estimates(&self) -> Result<(), Error> {
        let mut locked_fee_rate_cache = self.fee_rate_cache.write().unwrap();

        let confirmation_targets = vec![
            ConfirmationTarget::Background,
            ConfirmationTarget::Normal,
            ConfirmationTarget::HighPriority,
        ];
        for target in confirmation_targets {
            let num_blocks = match target {
                ConfirmationTarget::Background => 12,
                ConfirmationTarget::Normal => 6,
                ConfirmationTarget::HighPriority => 3,
            };

            let est_fee_rate = self.blockchain.estimate_fee(num_blocks).await;

            match est_fee_rate {
                Ok(rate) => {
                    locked_fee_rate_cache.insert(target, rate);
                    tracing::trace!(
                        self.logger,
                        "Fee rate estimation updated: {} sats/kwu",
                        rate.fee_wu(1000)
                    );
                }
                Err(e) => {
                    tracing::error!(self.logger, "Failed to update fee rate estimation: {}", e);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn create_funding_transaction(
        &self,
        output_script: Script,
        value_sats: u64,
        confirmation_target: ConfirmationTarget,
    ) -> Result<Transaction, Error> {
        let fee_rate = self.estimate_fee_rate(confirmation_target);

        let locked_wallet = self.inner.lock().unwrap();
        let mut tx_builder = locked_wallet.build_tx();

        tx_builder
            .add_recipient(output_script, value_sats)
            .fee_rate(fee_rate)
            .enable_rbf();

        let mut psbt = match tx_builder.finish() {
            Ok((psbt, _)) => {
                tracing::trace!(self.logger, "Created funding PSBT: {:?}", psbt);
                psbt
            }
            Err(err) => {
                tracing::error!(self.logger, "Failed to create funding transaction: {}", err);
                return Err(err.into());
            }
        };

        match locked_wallet.sign(&mut psbt, SignOptions::default()) {
            Ok(finalized) => {
                if !finalized {
                    return Err(Error::OnchainTxCreationFailed);
                }
            }
            Err(err) => {
                tracing::error!(self.logger, "Failed to create funding transaction: {}", err);
                return Err(err.into());
            }
        }

        Ok(psbt.extract_tx())
    }

    pub(crate) fn get_new_address(&self) -> Result<bitcoin::Address, Error> {
        let address_info = self.inner.lock().unwrap().get_address(AddressIndex::New)?;
        Ok(address_info.address)
    }

    pub(crate) fn get_balance(&self) -> Result<bdk::Balance, Error> {
        Ok(self.inner.lock().unwrap().get_balance()?)
    }

    /// Send funds to the given address.
    ///
    /// If `amount_msat_or_drain` is `None` the wallet will be drained, i.e., all available funds
    /// will be spent.
    pub(crate) fn send_to_address(
        &self,
        address: &bitcoin::Address,
        amount_msat_or_drain: Option<u64>,
    ) -> Result<Txid, Error> {
        let confirmation_target = ConfirmationTarget::Normal;
        let fee_rate = self.estimate_fee_rate(confirmation_target);

        let tx = {
            let locked_wallet = self.inner.lock().unwrap();
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
                    tracing::trace!(self.logger, "Created PSBT: {:?}", psbt);
                    psbt
                }
                Err(err) => {
                    tracing::error!(self.logger, "Failed to create transaction: {}", err);
                    return Err(err.into());
                }
            };

            match locked_wallet.sign(&mut psbt, SignOptions::default()) {
                Ok(finalized) => {
                    if !finalized {
                        return Err(Error::OnchainTxCreationFailed);
                    }
                }
                Err(err) => {
                    tracing::error!(self.logger, "Failed to create transaction: {}", err);
                    return Err(err.into());
                }
            }
            psbt.extract_tx()
        };

        self.broadcast_transaction(&tx);

        let txid = tx.txid();

        if let Some(amount_sats) = amount_msat_or_drain {
            tracing::info!(
                self.logger,
                "Created new transaction {} sending {}sats on-chain to address {}",
                txid,
                amount_sats,
                address
            );
        } else {
            tracing::info!(
                self.logger,
                "Created new transaction {} sending all available on-chain funds to address {}",
                txid,
                address
            );
        }

        Ok(txid)
    }

    fn estimate_fee_rate(&self, confirmation_target: ConfirmationTarget) -> FeeRate {
        let locked_fee_rate_cache = self.fee_rate_cache.read().unwrap();

        let fallback_sats_kwu = match confirmation_target {
            ConfirmationTarget::Background => FEERATE_FLOOR_SATS_PER_KW,
            ConfirmationTarget::Normal => 2000,
            ConfirmationTarget::HighPriority => 5000,
        };

        // We'll fall back on this, if we really don't have any other information.
        let fallback_rate = FeeRate::from_sat_per_kwu(fallback_sats_kwu as f32);

        *locked_fee_rate_cache
            .get(&confirmation_target)
            .unwrap_or(&fallback_rate)
    }
}

impl<D> FeeEstimator for Wallet<D>
where
    D: BatchDatabase,
{
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        (self.estimate_fee_rate(confirmation_target).fee_wu(1000) as u32)
            .max(FEERATE_FLOOR_SATS_PER_KW)
    }
}

impl<D> BroadcasterInterface for Wallet<D>
where
    D: BatchDatabase,
{
    fn broadcast_transaction(&self, tx: &Transaction) {
        let locked_runtime = self.runtime.read().unwrap();
        if locked_runtime.as_ref().is_none() {
            tracing::error!(self.logger, "Failed to broadcast transaction: No runtime.");
            return;
        }

        let res = tokio::task::block_in_place(move || {
            locked_runtime
                .as_ref()
                .unwrap()
                .block_on(async move { self.blockchain.broadcast(tx).await })
        });

        match res {
            Ok(_) => {}
            Err(err) => {
                tracing::error!(self.logger, "Failed to broadcast transaction: {}", err);
            }
        }
    }
}

/// Similar to [`KeysManager`], but overrides the destination and shutdown scripts so they are
/// directly spendable by the BDK wallet.
pub struct WalletKeysManager<D>
where
    D: BatchDatabase,
{
    inner: KeysManager,
    wallet: Arc<Wallet<D>>,
}

impl<D> WalletKeysManager<D>
where
    D: BatchDatabase,
{
    /// Constructs a `WalletKeysManager` that overrides the destination and shutdown scripts.
    ///
    /// See [`KeysManager::new`] for more information on `seed`, `starting_time_secs`, and
    /// `starting_time_nanos`.
    pub fn new(
        seed: &[u8; 32],
        starting_time_secs: u64,
        starting_time_nanos: u32,
        wallet: Arc<Wallet<D>>,
    ) -> Self {
        let inner = KeysManager::new(seed, starting_time_secs, starting_time_nanos);
        Self { inner, wallet }
    }

    /// See [`KeysManager::spend_spendable_outputs`] for documentation on this method.
    pub fn spend_spendable_outputs<C: Signing>(
        &self,
        descriptors: &[&SpendableOutputDescriptor],
        outputs: Vec<TxOut>,
        change_destination_script: Script,
        feerate_sat_per_1000_weight: u32,
        secp_ctx: &Secp256k1<C>,
    ) -> Result<Transaction, ()> {
        let only_non_static = &descriptors
            .iter()
            .filter(|desc| !matches!(desc, SpendableOutputDescriptor::StaticOutput { .. }))
            .copied()
            .collect::<Vec<_>>();
        self.inner.spend_spendable_outputs(
            only_non_static,
            outputs,
            change_destination_script,
            feerate_sat_per_1000_weight,
            secp_ctx,
        )
    }
}
