use anyhow::Context;
use anyhow::Result;
use bdk::blockchain::ElectrumBlockchain;
use bdk::sled;
use bdk::wallet::AddressIndex;
use bdk::TransactionDetails;
use bitcoin::secp256k1::All;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use bitcoin::Block;
use bitcoin::BlockHeader;
use bitcoin::KeyPair;
use bitcoin::Network;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::TxOut;
use bitcoin::Txid;
use dlc_manager::error::Error;
use dlc_manager::error::Error::WalletError;
use dlc_manager::Signer;
use dlc_manager::Utxo;
use dlc_sled_storage_provider::SledStorageProvider;
use lightning::chain::Filter;
use lightning::chain::WatchedOutput;
use simple_wallet::WalletStorage;
use std::sync::Arc;

/// This is a wrapper type introduced to be able to implement traits from `rust-dlc` on the
/// `bdk_ldk::LightningWallet`.
///
/// We want to eventually get rid of the dependency on `bdk-ldk`, because it's a dead project.
pub struct LnDlcWallet {
    ln_wallet: bdk_ldk::LightningWallet<ElectrumBlockchain, sled::Tree>,
    storage: Arc<SledStorageProvider>,
    secp: Secp256k1<All>,
}

impl LnDlcWallet {
    pub fn new(
        blockchain_client: Box<ElectrumBlockchain>,
        wallet: bdk::Wallet<bdk::sled::Tree>,
        storage: Arc<SledStorageProvider>,
    ) -> Self {
        Self {
            ln_wallet: bdk_ldk::LightningWallet::new(blockchain_client, wallet),
            storage,
            secp: Secp256k1::new(),
        }
    }

    // TODO: Better to keep this private and expose the necessary APIs instead.
    pub(crate) fn inner(&self) -> &bdk_ldk::LightningWallet<ElectrumBlockchain, sled::Tree> {
        &self.ln_wallet
    }

    pub(crate) fn tip(&self) -> Result<(u32, BlockHeader)> {
        let (height, header) = self.ln_wallet.get_tip()?;
        Ok((height, header))
    }

    /// A list of on-chain transactions. Transactions are sorted with the most recent transactions
    /// appearing first.
    ///
    /// This list won't be up-to-date unless the wallet has previously been synchronised with the
    /// blockchain.
    pub(crate) fn on_chain_transactions(&self) -> Result<Vec<TransactionDetails>> {
        let mut txs = self
            .ln_wallet
            .get_wallet()
            .context("Can't acquire lock on bdk_ldk wallet")?
            .list_transactions(false)
            .context("bar")?;

        txs.sort_by(|a, b| {
            b.confirmation_time
                .as_ref()
                .map(|t| t.height)
                .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
        });

        Ok(txs)
    }
}

impl dlc_manager::Blockchain for LnDlcWallet {
    fn send_transaction(&self, transaction: &Transaction) -> Result<(), Error> {
        self.ln_wallet
            .broadcast(transaction)
            .map_err(|_| Error::BlockchainError)
    }

    fn get_network(&self) -> Result<Network, Error> {
        let network = self
            .ln_wallet
            .get_wallet()
            .map_err(|e| WalletError(Box::new(e)))?
            .network();
        Ok(network)
    }

    fn get_blockchain_height(&self) -> Result<u64, Error> {
        Ok(self
            .ln_wallet
            .get_tip()
            .map_err(|_| Error::BlockchainError)?
            .0 as u64)
    }

    fn get_block_at_height(&self, _height: u64) -> Result<Block, Error> {
        todo!()
    }

    fn get_transaction(&self, _txid: &Txid) -> Result<Transaction, Error> {
        todo!()
    }

    fn get_transaction_confirmations(&self, _txid: &Txid) -> Result<u32, Error> {
        todo!()
    }
}

impl Filter for LnDlcWallet {
    fn register_tx(&self, txid: &Txid, script_pubkey: &Script) {
        self.inner().register_tx(txid, script_pubkey)
    }

    fn register_output(&self, output: WatchedOutput) {
        self.inner().register_output(output);
    }
}

impl Signer for LnDlcWallet {
    fn sign_tx_input(
        &self,
        _tx: &mut Transaction,
        _input_index: usize,
        _tx_out: &TxOut,
        _redeem_script: Option<Script>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn get_secret_key_for_pubkey(&self, pubkey: &PublicKey) -> Result<SecretKey, Error> {
        self.storage
            .get_priv_key_for_pubkey(pubkey)?
            .ok_or_else(|| Error::StorageError("No sk for provided pk".to_string()))
    }
}

impl dlc_manager::Wallet for LnDlcWallet {
    fn get_new_address(&self) -> Result<Address, Error> {
        let address_info = self
            .ln_wallet
            .get_wallet()
            .unwrap()
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError(Box::new(e)))?;
        Ok(address_info.address)
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

impl lightning::chain::chaininterface::BroadcasterInterface for LnDlcWallet {
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
