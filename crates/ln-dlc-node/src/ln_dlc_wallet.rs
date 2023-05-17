use crate::ldk_node_wallet;
use crate::seed::Bip39Seed;
use crate::TracingLogger;
use anyhow::Result;
use bdk::blockchain::esplora;
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
use lightning_transaction_sync::EsploraSyncClient;
use rust_bitcoin_coin_selection::select_coins;
use simple_wallet::WalletStorage;
use std::sync::Arc;

/// The 'stop gap' parameter used by BDK's wallet sync. This seems to configure the threshold
/// number of blocks after which BDK stops looking for scripts belonging to the wallet.
/// Note: This constant and value was copied from ldk_node
const BDK_CLIENT_STOP_GAP: usize = 20;
/// The number of concurrent requests made against the API provider.
/// Note: This constant and value was copied from ldk_node
const BDK_CLIENT_CONCURRENCY: u8 = 8;

/// This is a wrapper type introduced to be able to implement traits from `rust-dlc` on the
/// `ldk_node::LightningWallet`.
pub struct LnDlcWallet {
    ln_wallet: Arc<ldk_node_wallet::Wallet<sled::Tree>>,
    storage: Arc<SledStorageProvider>,
    secp: Secp256k1<All>,
    seed: Bip39Seed,
    runtime_handle: tokio::runtime::Handle,
}

impl LnDlcWallet {
    pub fn new(
        esplora_client: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
        on_chain_wallet: bdk::Wallet<bdk::sled::Tree>,
        storage: Arc<SledStorageProvider>,
        seed: Bip39Seed,
        runtime_handle: tokio::runtime::Handle,
    ) -> Self {
        let blockchain =
            EsploraBlockchain::from_client(esplora_client.client().clone(), BDK_CLIENT_STOP_GAP)
                .with_concurrency(BDK_CLIENT_CONCURRENCY);

        let wallet = Arc::new(ldk_node_wallet::Wallet::new(
            blockchain,
            on_chain_wallet,
            runtime_handle.clone(),
        ));

        Self {
            ln_wallet: wallet,
            storage,
            secp: Secp256k1::new(),
            seed,
            runtime_handle,
        }
    }

    pub(crate) fn get_seed_phrase(&self) -> Vec<String> {
        self.seed.get_seed_phrase()
    }

    // TODO: Better to keep this private and expose the necessary APIs instead.
    pub(crate) fn inner(&self) -> Arc<ldk_node_wallet::Wallet<sled::Tree>> {
        self.ln_wallet.clone()
    }

    pub(crate) async fn tip(&self) -> Result<(u32, BlockHash)> {
        let (height, header) = self.ln_wallet.tip()?;
        Ok((height, header))
    }

    /// A list of on-chain transactions. Transactions are sorted with the most recent transactions
    /// appearing first.
    ///
    /// This list won't be up-to-date unless the wallet has previously been synchronised with the
    /// blockchain.
    pub(crate) async fn on_chain_transactions(&self) -> Result<Vec<TransactionDetails>> {
        let mut txs = self.ln_wallet.on_chain_transaction_list().await?;

        txs.sort_by(|a, b| {
            b.confirmation_time
                .as_ref()
                .map(|t| t.height)
                .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
        });

        Ok(txs)
    }

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

    fn get_network(&self) -> Result<Network, Error> {
        self.ln_wallet
            .network()
            .map_err(|e| Error::BlockchainError(e.to_string()))
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
        let block = tokio::task::block_in_place(move || {
            self.runtime_handle.block_on(async {
                let block_hash = self
                    .ln_wallet
                    .blockchain
                    .get_block_hash(height as u32)
                    .await?;
                let block = self
                    .ln_wallet
                    .blockchain
                    .get_block_by_hash(&block_hash)
                    .await?;

                Result::<_, esplora::EsploraError>::Ok(block)
            })
        })
        .map_err(|e| {
            Error::BlockchainError(format!("Could not find block at height {height}: {e:#}"))
        })?
        .ok_or_else(|| {
            Error::BlockchainError(format!("Could not find block at height {height}"))
        })?;

        Ok(block)
    }

    fn get_transaction(&self, txid: &Txid) -> Result<Transaction, Error> {
        tokio::task::block_in_place(move || {
            self.runtime_handle.block_on(async {
                self.ln_wallet
                    .blockchain
                    .get_tx(txid)
                    .await
                    .map_err(|e| {
                        Error::BlockchainError(format!("Could not find transaction {txid}: {e:#}"))
                    })?
                    .ok_or_else(|| {
                        Error::BlockchainError(format!("Could not get transaction body {txid}"))
                    })
            })
        })
    }

    fn get_transaction_confirmations(&self, _txid: &Txid) -> Result<u32, Error> {
        unreachable!("This function is not meant to be called by us")
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
        let address = self
            .ln_wallet
            .get_new_address()
            .map_err(|e| Error::BlockchainError(e.to_string()))?;
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

#[derive(Clone)]
struct UtxoWrap {
    utxo: Utxo,
}

impl rust_bitcoin_coin_selection::Utxo for UtxoWrap {
    fn get_value(&self) -> u64 {
        self.utxo.tx_out.value
    }
}
