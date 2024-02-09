use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ldk_node_wallet;
use crate::node::Storage;
use crate::storage::TenTenOneStorage;
use crate::TracingLogger;
use crate::WalletSettings;
use anyhow::anyhow;
use anyhow::Result;
use bdk::blockchain::EsploraBlockchain;
use bdk::esplora_client::TxStatus;
use bdk::sled;
use bdk::SignOptions;
use bdk::TransactionDetails;
use bitcoin::psbt::PartiallySignedTransaction;
use bitcoin::secp256k1::All;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use bitcoin::Block;
use bitcoin::BlockHash;
use bitcoin::KeyPair;
use bitcoin::Network;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::Txid;
use dlc_manager::error::Error;
use dlc_manager::Blockchain;
use dlc_manager::Signer;
use dlc_manager::Utxo;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning_transaction_sync::EsploraSyncClient;
use ln_dlc_storage::DlcStorageProvider;
use ln_dlc_storage::WalletStorage;
use parking_lot::RwLock;
use std::sync::Arc;

/// This is a wrapper type introduced to be able to implement traits from `rust-dlc` on the
/// `ldk_node::LightningWallet`.
pub struct LnDlcWallet<S, N> {
    ln_wallet: Arc<ldk_node_wallet::Wallet<sled::Tree, EsploraBlockchain, FeeRateEstimator, N>>,
    dlc_storage: Arc<DlcStorageProvider<S>>,
    secp: Secp256k1<All>,
    network: Network,
    /// Cache for the last unused address according to the latest on-chain sync.
    ///
    /// We can run into address reuse if we access this value multiple times between syncs. This is
    /// acceptable as the current alternative is to block the caller until the sync ends, which can
    /// have much more severe consequences.
    address_cache: RwLock<Address>,
}

impl<S: TenTenOneStorage, N: Storage> LnDlcWallet<S, N> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        esplora_client: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
        on_chain_wallet: bdk::Wallet<bdk::sled::Tree>,
        fee_rate_estimator: Arc<FeeRateEstimator>,
        dlc_storage: Arc<DlcStorageProvider<S>>,
        node_storage: Arc<N>,
        bdk_client_stop_gap: usize,
        bdk_client_concurrency: u8,
        settings: WalletSettings,
    ) -> Self {
        let blockchain =
            EsploraBlockchain::from_client(esplora_client.client().clone(), bdk_client_stop_gap)
                .with_concurrency(bdk_client_concurrency);

        let network = on_chain_wallet.network();

        let wallet = Arc::new(ldk_node_wallet::Wallet::new(
            blockchain,
            on_chain_wallet,
            fee_rate_estimator,
            node_storage,
            settings,
        ));

        let last_unused_address = wallet
            .get_last_unused_address()
            .expect("to get the last unused address");

        Self {
            ln_wallet: wallet,
            dlc_storage,
            secp: Secp256k1::new(),
            network,
            address_cache: RwLock::new(last_unused_address),
        }
    }

    pub fn ldk_wallet(
        &self,
    ) -> Arc<ldk_node_wallet::Wallet<sled::Tree, EsploraBlockchain, FeeRateEstimator, N>> {
        self.ln_wallet.clone()
    }

    pub fn tip(&self) -> Result<(u32, BlockHash)> {
        let (height, header) = self.ln_wallet.tip()?;
        Ok((height, header))
    }

    /// A list of on-chain transactions. Transactions are sorted with the most recent transactions
    /// appearing first.
    ///
    /// This list won't be up-to-date unless the wallet has previously been synchronised with the
    /// blockchain.
    pub fn on_chain_transactions(&self) -> Result<Vec<TransactionDetails>> {
        let mut txs = self.ln_wallet.on_chain_transaction_list()?;

        txs.sort_by(|a, b| {
            b.confirmation_time
                .as_ref()
                .map(|t| t.height)
                .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
        });

        Ok(txs)
    }

    pub fn unused_address(&self) -> Address {
        self.address_cache.read().clone()
    }

    pub fn update_lookahead(&self, gap: u32) -> Result<bool> {
        self.ldk_wallet().update_lookahead(gap)
    }

    pub fn new_address(&self) -> Result<Address> {
        self.ldk_wallet().get_new_address()
    }

    pub fn is_mine(&self, script: &Script) -> Result<bool> {
        self.ldk_wallet().is_mine(script)
    }

    pub fn sync_and_update_address_cache(&self) -> Result<()> {
        self.ldk_wallet().sync()?;

        self.update_address_cache()?;

        Ok(())
    }

    pub fn get_transaction(&self, txid: &Txid) -> Result<Transaction> {
        self.ln_wallet
            .blockchain
            .get_tx(txid)
            .map_err(|e| anyhow!("Could not get trasaction {txid}: {e:#}"))?
            .ok_or_else(|| anyhow!("Transaction {txid} not found on-chain"))
    }

    fn update_address_cache(&self) -> Result<()> {
        let address = self.ldk_wallet().get_last_unused_address()?;
        *self.address_cache.write() = address;

        Ok(())
    }
}

impl<S: TenTenOneStorage, N: Storage> Blockchain for LnDlcWallet<S, N> {
    fn send_transaction(&self, transaction: &Transaction) -> Result<(), Error> {
        self.ln_wallet
            .broadcast_transaction(transaction)
            .map_err(|e| Error::WalletError(e.into()))?;

        Ok(())
    }

    fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network)
    }

    fn get_blockchain_height(&self) -> Result<u64, Error> {
        let height = self
            .ln_wallet
            .tip()
            .map_err(|e| Error::BlockchainError(e.to_string()))?
            .0;
        Ok(height as u64)
    }

    fn get_block_at_height(&self, height: u64) -> Result<Block, Error> {
        let block_hash = self
            .ln_wallet
            .blockchain
            .get_block_hash(height as u32)
            .map_err(|e| {
                Error::BlockchainError(format!("Could not find block at height {height}: {e:#}"))
            })?;
        let block = self
            .ln_wallet
            .blockchain
            .get_block_by_hash(&block_hash)
            .map_err(|e| {
                Error::BlockchainError(format!("Could not find block at height {height}: {e:#}"))
            })?
            .ok_or_else(|| {
                Error::BlockchainError(format!("Could not find block at height {height}"))
            })?;

        Ok(block)
    }

    fn get_transaction(&self, txid: &Txid) -> Result<Transaction, Error> {
        self.get_transaction(txid).map_err(|e| {
            Error::BlockchainError(format!("Could not find transaction {txid}: {e:#}"))
        })
    }

    fn get_transaction_confirmations(&self, txid: &Txid) -> Result<u32, Error> {
        let confirmation_height = match self
            .ln_wallet
            .blockchain
            .get_tx_status(txid)
            .map_err(|e| Error::BlockchainError(e.to_string()))?
        {
            Some(TxStatus {
                block_height: Some(height),
                ..
            }) => height,
            _ => return Ok(0),
        };

        let tip = self
            .ln_wallet
            .blockchain
            .get_height()
            .map_err(|e| Error::BlockchainError(e.to_string()))?;
        let confirmations = tip.checked_sub(confirmation_height).unwrap_or_default();

        Ok(confirmations)
    }
}

impl<S: TenTenOneStorage, N: Storage> Signer for LnDlcWallet<S, N> {
    fn get_secret_key_for_pubkey(&self, pubkey: &PublicKey) -> Result<SecretKey, Error> {
        self.dlc_storage
            .get_priv_key_for_pubkey(pubkey)
            .map_err(|e| Error::StorageError(format!("Couldn't get priv key for pubkey. {e:#}")))?
            .ok_or_else(|| Error::StorageError("No sk for provided pk".to_string()))
    }

    fn sign_psbt_input(
        &self,
        psbt: &mut PartiallySignedTransaction,
        _: usize,
    ) -> std::result::Result<(), Error> {
        let ldk_wallet = &self.ldk_wallet();
        let bdk = ldk_wallet.bdk_lock();

        bdk.sign(
            psbt,
            SignOptions {
                trust_witness_utxo: true,
                ..Default::default()
            },
        )
        .map_err(|e| Error::WalletError(e.into()))?;

        Ok(())
    }
}

impl<S: TenTenOneStorage, N: Storage> dlc_manager::Wallet for LnDlcWallet<S, N> {
    fn get_new_address(&self) -> Result<Address, Error> {
        Ok(self.unused_address())
    }

    fn get_new_secret_key(&self) -> Result<SecretKey, Error> {
        let kp = KeyPair::new(&self.secp, &mut rand::thread_rng());
        let sk = kp.secret_key();

        self.dlc_storage
            .upsert_key_pair(&kp.public_key(), &sk)
            .map_err(|e| Error::StorageError(format!("Failed to update key pair. {e:#}")))?;

        Ok(sk)
    }

    // This is only used to create the funding transaction of a DLC or a DLC channel.
    fn get_utxos_for_amount(
        &self,
        amount: u64,
        fee_rate: Option<u64>,
        lock_utxos: bool,
    ) -> Result<Vec<Utxo>, Error> {
        let utxos = self
            .ldk_wallet()
            .get_utxos_for_dlc_funding_transaction(amount, fee_rate, lock_utxos)
            .map_err(|error| {
                Error::InvalidState(format!("Could not find utxos for amount: {error:?}"))
            })?;

        Ok(utxos)
    }

    fn import_address(&self, _address: &Address) -> Result<(), Error> {
        Ok(())
    }
}

impl<S: TenTenOneStorage, N: Storage> BroadcasterInterface for LnDlcWallet<S, N> {
    fn broadcast_transactions(&self, txs: &[&Transaction]) {
        for tx in txs {
            if let Err(e) = self.ln_wallet.broadcast_transaction(tx) {
                tracing::error!(
                    txid = %tx.txid(),
                    "Error when broadcasting transaction: {e:#}"
                );
            }
        }
    }
}
