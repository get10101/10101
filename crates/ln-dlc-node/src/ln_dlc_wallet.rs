use crate::ldk_node_wallet;
use crate::seed::Bip39Seed;
use crate::TracingLogger;
use anyhow::Result;
use bdk::blockchain::EsploraBlockchain;
use bdk::sled;
use bdk::TransactionDetails;
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
use bitcoin::TxOut;
use bitcoin::Txid;
use dlc_manager::error::Error;
use dlc_manager::Blockchain;
use dlc_manager::Signer;
use dlc_manager::Utxo;
use dlc_sled_storage_provider::SledStorageProvider;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::Filter;
use lightning::chain::WatchedOutput;
use lightning_transaction_sync::EsploraSyncClient;
use simple_wallet::WalletStorage;
use std::sync::Arc;
use tokio::sync::RwLock;

// Copied from ldk_node
// The 'stop gap' parameter used by BDK's wallet sync. This seems to configure the threshold
// number of blocks after which BDK stops looking for scripts belonging to the wallet.
const BDK_CLIENT_STOP_GAP: usize = 20;
// Copied from ldk_node
// The number of concurrent requests made against the API provider.
const BDK_CLIENT_CONCURRENCY: u8 = 8;

/// This is a wrapper type introduced to be able to implement traits from `rust-dlc` on the
/// `ldk_node::LightningWallet`.
///
/// We want to eventually get rid of the dependency on `bdk-ldk`, because it's a dead project.
pub struct LnDlcWallet {
    ln_wallet: Arc<ldk_node_wallet::Wallet<sled::Tree>>,
    storage: Arc<SledStorageProvider>,
    secp: Secp256k1<All>,
    seed: Bip39Seed,
}

impl LnDlcWallet {
    pub fn new(
        tx_sync: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
        on_chain_wallet: bdk::Wallet<bdk::sled::Tree>,
        storage: Arc<SledStorageProvider>,
        seed: Bip39Seed,
    ) -> Self {
        // TODO: Upgrade bdk so we can agree on using esplora client 0.3.0!
        let blockchain =
            EsploraBlockchain::from_client(tx_sync.client().clone(), BDK_CLIENT_STOP_GAP)
                .with_concurrency(BDK_CLIENT_CONCURRENCY);

        // TODO: Can we get rid of this runtime?
        let runtime = Arc::new(RwLock::new(None));
        let wallet = Arc::new(ldk_node_wallet::Wallet::new(
            blockchain,
            on_chain_wallet,
            Arc::clone(&runtime),
        ));

        Self {
            ln_wallet: wallet,
            storage,
            secp: Secp256k1::new(),
            seed,
        }
    }

    pub(crate) fn get_seed_phrase(&self) -> Vec<String> {
        self.seed.get_seed_phrase()
    }

    // TODO: Better to keep this private and expose the necessary APIs instead.
    pub(crate) fn inner(&self) -> &ldk_node_wallet::Wallet<sled::Tree> {
        &self.ln_wallet
    }

    pub(crate) async fn tip(&self) -> Result<(u32, BlockHash)> {
        let (height, header) = self.ln_wallet.tip().await?;
        Ok((height, header))
    }

    /// A list of on-chain transactions. Transactions are sorted with the most recent transactions
    /// appearing first.
    ///
    /// This list won't be up-to-date unless the wallet has previously been synchronised with the
    /// blockchain.
    pub(crate) fn on_chain_transactions(&self) -> Result<Vec<TransactionDetails>> {
        let mut txs = self.ln_wallet.on_chain_transaction_list()?;

        txs.sort_by(|a, b| {
            b.confirmation_time
                .as_ref()
                .map(|t| t.height)
                .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
        });

        Ok(txs)
    }
}

impl Blockchain for LnDlcWallet {
    fn send_transaction(&self, transaction: &Transaction) -> Result<(), Error> {
        self.ln_wallet.broadcast_transaction(transaction);

        Ok(())
    }

    fn get_network(&self) -> Result<Network, Error> {
        self.ln_wallet
            .network()
            .map_err(|e| Error::BlockchainError(e.to_string()))
    }

    #[allow(useless_deprecated)]
    #[deprecated(
        since = "0.1.0",
        note = "We only use the on-chain wallet to determine the network and broadcast"
    )]
    fn get_blockchain_height(&self) -> Result<u64, Error> {
        unreachable!("This is not used")
    }

    #[allow(useless_deprecated)]
    #[deprecated(
        since = "0.1.0",
        note = "We only use the on-chain wallet to determine the network and broadcast"
    )]
    fn get_block_at_height(&self, _height: u64) -> Result<Block, Error> {
        unreachable!("This is not used")
    }

    #[allow(useless_deprecated)]
    #[deprecated(
        since = "0.1.0",
        note = "We only use the on-chain wallet to determine the network and broadcast"
    )]
    fn get_transaction(&self, _txid: &Txid) -> Result<Transaction, Error> {
        unreachable!("This is not used")
    }

    #[allow(useless_deprecated)]
    #[deprecated(
        since = "0.1.0",
        note = "We only use the on-chain wallet to determine the network and broadcast"
    )]
    fn get_transaction_confirmations(&self, _txid: &Txid) -> Result<u32, Error> {
        unreachable!("This is not used")
    }
}

// TODO: We can likely remove this; it is only used in the tests
// Not sure why this was implemented in the fist place
impl Filter for LnDlcWallet {
    fn register_tx(&self, txid: &Txid, script_pubkey: &Script) {
        unreachable!("We are not using this functionality")
    }

    fn register_output(&self, _output: WatchedOutput) {
        unreachable!("We are not using this functionality")
    }
}

impl Signer for LnDlcWallet {
    fn sign_tx_input(
        &self,
        tx: &mut Transaction,
        input_index: usize,
        tx_out: &TxOut,
        _: Option<Script>,
    ) -> Result<(), Error> {
        let address = Address::from_script(&tx_out.script_pubkey, self.get_network()?)
            .expect("a valid scriptpubkey");
        let seckey = self
            .storage
            .get_priv_key_for_address(&address)?
            .expect("to have the requested private key");
        dlc::util::sign_p2wpkh_input(
            &self.secp,
            &seckey,
            tx,
            input_index,
            bitcoin::EcdsaSighashType::All,
            tx_out.value,
        )?;
        Ok(())
    }

    fn get_secret_key_for_pubkey(&self, pubkey: &PublicKey) -> Result<SecretKey, Error> {
        self.storage
            .get_priv_key_for_pubkey(pubkey)?
            .ok_or_else(|| Error::StorageError("No sk for provided pk".to_string()))
    }
}

impl dlc_manager::Wallet for LnDlcWallet {
    fn get_new_address(&self) -> Result<Address, Error> {
        // TODO: Fix unwrap: error mapping
        let address = self.ln_wallet.get_new_address().unwrap();
        Ok(address)
    }

    fn get_new_secret_key(&self) -> Result<SecretKey, Error> {
        let kp = KeyPair::new(&self.secp, &mut rand::thread_rng());
        let sk = kp.secret_key();

        self.storage.upsert_key_pair(&kp.public_key(), &sk)?;

        Ok(sk)
    }

    fn get_utxos_for_amount(
        &self,
        _amount: u64,
        _fee_rate: Option<u64>,
        _lock_utxos: bool,
    ) -> Result<Vec<Utxo>, Error> {
        todo!()
    }

    fn import_address(&self, _address: &Address) -> Result<(), Error> {
        Ok(())
    }
}

impl BroadcasterInterface for LnDlcWallet {
    fn broadcast_transaction(&self, tx: &Transaction) {
        self.ln_wallet.broadcast_transaction(tx)
    }
}

impl lightning::chain::chaininterface::FeeEstimator for LnDlcWallet {
    fn get_est_sat_per_1000_weight(
        &self,
        confirmation_target: lightning::chain::chaininterface::ConfirmationTarget,
    ) -> u32 {
        self.ln_wallet
            .get_est_sat_per_1000_weight(confirmation_target)
    }
}
