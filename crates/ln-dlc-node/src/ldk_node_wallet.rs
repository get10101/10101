use crate::fee_rate_estimator::EstimateFeeRate;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use autometrics::autometrics;
use bdk::blockchain::Blockchain;
use bdk::blockchain::GetBlockHash;
use bdk::blockchain::GetHeight;
use bdk::database::BatchDatabase;
use bdk::wallet::AddressIndex;
use bdk::FeeRate;
use bdk::SignOptions;
use bdk::SyncOptions;
use bdk::TransactionDetails;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::BlockHash;
use bitcoin::OutPoint;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::Txid;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use parking_lot::Mutex;
use parking_lot::MutexGuard;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

pub struct Wallet<D, B, F>
where
    D: BatchDatabase,
    B: Blockchain,
    F: EstimateFeeRate,
{
    // A BDK blockchain used for wallet sync.
    pub(crate) blockchain: Arc<B>,
    // A BDK on-chain wallet.
    inner: Mutex<bdk::Wallet<D>>,
    settings: RwLock<WalletSettings>,
    fee_rate_estimator: Arc<F>,
    locked_outpoints: Mutex<Vec<OutPoint>>,
}

#[derive(Clone, Debug, Default)]
pub struct WalletSettings {
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,
}

impl<D, B, F> Wallet<D, B, F>
where
    D: BatchDatabase,
    B: Blockchain,
    F: EstimateFeeRate,
{
    pub(crate) fn new(blockchain: B, wallet: bdk::Wallet<D>, fee_rate_estimator: Arc<F>) -> Self {
        let inner = Mutex::new(wallet);
        let settings = RwLock::new(WalletSettings::default());

        Self {
            blockchain: Arc::new(blockchain),
            inner,
            settings,
            fee_rate_estimator,
            locked_outpoints: Mutex::new(vec![]),
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
    pub fn sync(&self) -> Result<()> {
        let wallet_lock = self.bdk_lock();

        let now = Instant::now();

        tracing::info!("Started on-chain sync");

        wallet_lock.sync(&self.blockchain, SyncOptions::default())?;

        tracing::info!(
            duration = now.elapsed().as_millis(),
            "Finished on-chain sync",
        );

        self.locked_outpoints.lock().clear();

        Ok(())
    }

    pub fn get_fee_rate(&self, confirmation_target: ConfirmationTarget) -> FeeRate {
        self.fee_rate_estimator.estimate(confirmation_target)
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

        let fee_rate = self.get_fee_rate(confirmation_target);
        tx_builder
            .add_recipient(output_script, value_sats)
            .fee_rate(fee_rate)
            .enable_rbf();

        let mut locked_outpoints = self.locked_outpoints.lock();
        for outpoint in locked_outpoints.iter() {
            tx_builder.add_unspendable(*outpoint);
        }

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

        let transaction = psbt.extract_tx();

        let prev_outpoints = transaction
            .input
            .iter()
            .map(|input| input.previous_output)
            .collect::<Vec<_>>();

        locked_outpoints.extend(prev_outpoints);

        Ok(transaction)
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
        let fee_rate = self.fee_rate_estimator.estimate(ConfirmationTarget::Normal);

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
        let hash = self.blockchain.get_block_hash(height as u64)?;

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

impl<D, B, F> BroadcasterInterface for Wallet<D, B, F>
where
    D: BatchDatabase,
    B: Blockchain,
    F: EstimateFeeRate,
{
    fn broadcast_transaction(&self, tx: &Transaction) {
        let txid = tx.txid();

        tracing::info!(%txid, raw_tx = %serialize_hex(&tx), "Broadcasting transaction");

        if let Err(err) = self.blockchain.broadcast(tx) {
            tracing::error!("Failed to broadcast transaction: {err:#}");
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::fee_rate_estimator::EstimateFeeRate;
    use crate::ldk_node_wallet::Wallet;
    use anyhow::Result;
    use bdk::blockchain::Blockchain;
    use bdk::blockchain::Capability;
    use bdk::blockchain::GetBlockHash;
    use bdk::blockchain::GetHeight;
    use bdk::blockchain::GetTx;
    use bdk::blockchain::Progress;
    use bdk::blockchain::WalletSync;
    use bdk::database::BatchDatabase;
    use bdk::populate_test_db;
    use bdk::testutils;
    use bdk::BlockTime;
    use bdk::Error;
    use bdk::FeeRate;
    use bitcoin::util::bip32::ExtendedPrivKey;
    use bitcoin::Amount;
    use bitcoin::BlockHash;
    use bitcoin::Network;
    use bitcoin::Script;
    use bitcoin::Transaction;
    use bitcoin::Txid;
    use lightning::chain::chaininterface::ConfirmationTarget;
    use rand::thread_rng;
    use rand::CryptoRng;
    use rand::RngCore;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::sync::Arc;

    #[tokio::test]
    async fn wallet_with_two_utxo_should_be_able_to_fund_twice_but_not_three_times() {
        let mut rng = thread_rng();
        let test_wallet = new_test_wallet(&mut rng, Amount::from_btc(1.0).unwrap(), 2).unwrap();
        let wallet = Wallet::new(DummyEsplora, test_wallet, Arc::new(DummyFeeRateEstimator));

        let _ = wallet
            .create_funding_transaction(
                Script::new(),
                Amount::from_btc(0.5).unwrap().to_sat(),
                ConfirmationTarget::Background,
            )
            .await
            .unwrap();
        let _ = wallet
            .create_funding_transaction(
                Script::new(),
                Amount::from_btc(0.5).unwrap().to_sat(),
                ConfirmationTarget::Background,
            )
            .await
            .unwrap();
        assert!(wallet
            .create_funding_transaction(
                Script::new(),
                Amount::from_btc(0.5).unwrap().to_sat(),
                ConfirmationTarget::Background,
            )
            .await
            .is_err());
    }

    fn new_test_wallet(
        rng: &mut (impl RngCore + CryptoRng),
        utxo_amount: Amount,
        num_utxos: u8,
    ) -> Result<bdk::Wallet<bdk::database::MemoryDatabase>> {
        new_test_wallet_from_database(
            rng,
            utxo_amount,
            num_utxos,
            bdk::database::MemoryDatabase::new(),
        )
    }

    fn new_test_wallet_from_database<DB: BatchDatabase>(
        rng: &mut (impl RngCore + CryptoRng),
        utxo_amount: Amount,
        num_utxos: u8,
        mut database: DB,
    ) -> Result<bdk::Wallet<DB>> {
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);

        let key = ExtendedPrivKey::new_master(Network::Regtest, &seed)?;
        let descriptors = testutils!(@descriptors (&format!("wpkh({key}/*)")));

        for index in 0..num_utxos {
            populate_test_db!(
                &mut database,
                testutils! {
                    @tx ( (@external descriptors, index as u32) => utxo_amount.to_sat() ) (@confirmations 1)
                },
                Some(100)
            );
        }

        let wallet = bdk::Wallet::new(&descriptors.0, None, Network::Regtest, database)?;

        Ok(wallet)
    }

    struct DummyEsplora;
    struct DummyFeeRateEstimator;

    impl EstimateFeeRate for DummyFeeRateEstimator {
        fn estimate(&self, _: ConfirmationTarget) -> FeeRate {
            FeeRate::from_sat_per_vb(1.0)
        }
    }

    impl WalletSync for DummyEsplora {
        fn wallet_setup<D: BatchDatabase>(
            &self,
            _: &RefCell<D>,
            _: Box<dyn Progress>,
        ) -> std::result::Result<(), Error> {
            unimplemented!()
        }
    }

    impl GetHeight for DummyEsplora {
        fn get_height(&self) -> std::result::Result<u32, Error> {
            unimplemented!()
        }
    }

    impl GetTx for DummyEsplora {
        fn get_tx(&self, _: &Txid) -> std::result::Result<Option<Transaction>, Error> {
            unimplemented!()
        }
    }

    impl GetBlockHash for DummyEsplora {
        fn get_block_hash(&self, _: u64) -> std::result::Result<BlockHash, Error> {
            unimplemented!()
        }
    }

    impl Blockchain for DummyEsplora {
        fn get_capabilities(&self) -> HashSet<Capability> {
            unimplemented!()
        }

        fn broadcast(&self, _: &Transaction) -> std::result::Result<(), Error> {
            unimplemented!()
        }

        fn estimate_fee(&self, _: usize) -> std::result::Result<FeeRate, Error> {
            unimplemented!()
        }
    }
}
