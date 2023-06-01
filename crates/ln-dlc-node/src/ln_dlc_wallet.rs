use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ldk_node_wallet;
use crate::seed::Bip39Seed;
use crate::TracingLogger;
use anyhow::Result;
use autometrics::autometrics;
use bdk::blockchain::EsploraBlockchain;
use bdk::esplora_client::TxStatus;
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
use lightning_transaction_sync::EsploraSyncClient;
use rust_bitcoin_coin_selection::select_coins;
use simple_wallet::WalletStorage;
use std::sync::Arc;

/// This is a wrapper type introduced to be able to implement traits from `rust-dlc` on the
/// `ldk_node::LightningWallet`.
pub struct LnDlcWallet {
    ln_wallet: Arc<ldk_node_wallet::Wallet<sled::Tree>>,
    storage: Arc<SledStorageProvider>,
    secp: Secp256k1<All>,
    seed: Bip39Seed,
    network: Network,
}

impl LnDlcWallet {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        esplora_client: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
        on_chain_wallet: bdk::Wallet<bdk::sled::Tree>,
        fee_rate_estimator: Arc<FeeRateEstimator>,
        storage: Arc<SledStorageProvider>,
        seed: Bip39Seed,
        bdk_client_stop_gap: usize,
        bdk_client_concurrency: u8,
    ) -> Self {
        let blockchain =
            EsploraBlockchain::from_client(esplora_client.client().clone(), bdk_client_stop_gap)
                .with_concurrency(bdk_client_concurrency);

        let network = on_chain_wallet.network();

        let wallet = Arc::new(ldk_node_wallet::Wallet::new(
            blockchain,
            on_chain_wallet,
            esplora_client,
            fee_rate_estimator,
        ));

        Self {
            ln_wallet: wallet,
            storage,
            secp: Secp256k1::new(),
            seed,
            network,
        }
    }

    pub(crate) fn get_seed_phrase(&self) -> Vec<String> {
        self.seed.get_seed_phrase()
    }

    // TODO: Better to keep this private and expose the necessary APIs instead.
    pub(crate) fn inner(&self) -> Arc<ldk_node_wallet::Wallet<sled::Tree>> {
        self.ln_wallet.clone()
    }

    #[autometrics]
    pub(crate) fn tip(&self) -> Result<(u32, BlockHash)> {
        let (height, header) = self.ln_wallet.tip()?;
        Ok((height, header))
    }

    /// A list of on-chain transactions. Transactions are sorted with the most recent transactions
    /// appearing first.
    ///
    /// This list won't be up-to-date unless the wallet has previously been synchronised with the
    /// blockchain.
    #[autometrics]
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

    #[autometrics]
    pub fn get_last_unused_address(&self) -> Result<Address> {
        let address = self.inner().get_last_unused_address()?;

        Ok(address)
    }
}

impl Blockchain for LnDlcWallet {
    fn send_transaction(&self, transaction: &Transaction) -> Result<(), Error> {
        self.ln_wallet.broadcast_transaction(transaction);

        Ok(())
    }

    #[autometrics]
    fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network)
    }

    #[autometrics]
    fn get_blockchain_height(&self) -> Result<u64, Error> {
        let height = self
            .ln_wallet
            .tip()
            .map_err(|e| Error::BlockchainError(e.to_string()))?
            .0;
        Ok(height as u64)
    }

    #[autometrics]
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

    #[autometrics]
    fn get_transaction(&self, txid: &Txid) -> Result<Transaction, Error> {
        self.ln_wallet
            .blockchain
            .get_tx(txid)
            .map_err(|e| {
                Error::BlockchainError(format!("Could not find transaction {txid}: {e:#}"))
            })?
            .ok_or_else(|| Error::BlockchainError(format!("Could not get transaction body {txid}")))
    }

    #[autometrics]
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

impl Signer for LnDlcWallet {
    #[autometrics]
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

    #[autometrics]
    fn get_secret_key_for_pubkey(&self, pubkey: &PublicKey) -> Result<SecretKey, Error> {
        self.storage
            .get_priv_key_for_pubkey(pubkey)?
            .ok_or_else(|| Error::StorageError("No sk for provided pk".to_string()))
    }
}

impl dlc_manager::Wallet for LnDlcWallet {
    #[autometrics]
    fn get_new_address(&self) -> Result<Address, Error> {
        let address = self
            .ln_wallet
            .get_new_address()
            .map_err(|e| Error::BlockchainError(e.to_string()))?;
        Ok(address)
    }

    #[autometrics]
    fn get_new_secret_key(&self) -> Result<SecretKey, Error> {
        let kp = KeyPair::new(&self.secp, &mut rand::thread_rng());
        let sk = kp.secret_key();

        self.storage.upsert_key_pair(&kp.public_key(), &sk)?;

        Ok(sk)
    }

    #[autometrics]
    fn get_utxos_for_amount(
        &self,
        amount: u64,
        _: Option<u64>,
        lock_utxos: bool,
    ) -> Result<Vec<Utxo>, Error> {
        let mut utxos = self
            .storage
            .get_utxos()?
            .into_iter()
            .filter(|x| !x.reserved)
            .map(|x| UtxoWrap { utxo: x })
            .collect::<Vec<_>>();
        let selection = select_coins(amount, 20, &mut utxos)
            .ok_or_else(|| Error::InvalidState("Not enough fund in utxos".to_string()))?;
        if lock_utxos {
            for utxo in selection.clone() {
                let updated = Utxo {
                    reserved: true,
                    ..utxo.utxo
                };
                self.storage.upsert_utxo(&updated)?;
            }
        }
        Ok(selection.into_iter().map(|x| x.utxo).collect::<Vec<_>>())
    }

    #[autometrics]
    fn import_address(&self, _address: &Address) -> Result<(), Error> {
        Ok(())
    }
}

impl BroadcasterInterface for LnDlcWallet {
    #[autometrics]
    fn broadcast_transaction(&self, tx: &Transaction) {
        self.ln_wallet.broadcast_transaction(tx)
    }
}

#[derive(Clone)]
struct UtxoWrap {
    utxo: Utxo,
}

impl rust_bitcoin_coin_selection::Utxo for UtxoWrap {
    fn get_value(&self) -> u64 {
        self.utxo.tx_out.value
    }
}
