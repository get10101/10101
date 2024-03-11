use crate::bitcoin_conversion::to_address_29;
use crate::bitcoin_conversion::to_block_29;
use crate::bitcoin_conversion::to_network_29;
use crate::bitcoin_conversion::to_outpoint_29;
use crate::bitcoin_conversion::to_outpoint_30;
use crate::bitcoin_conversion::to_psbt_29;
use crate::bitcoin_conversion::to_psbt_30;
use crate::bitcoin_conversion::to_script_29;
use crate::bitcoin_conversion::to_tx_29;
use crate::bitcoin_conversion::to_tx_30;
use crate::bitcoin_conversion::to_txid_29;
use crate::bitcoin_conversion::to_txid_30;
use crate::bitcoin_conversion::to_txout_29;
use crate::blockchain::Blockchain;
use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::on_chain_wallet::OnChainWallet;
use crate::storage::TenTenOneStorage;
use anyhow::Result;
use bdk::LocalOutput;
use bdk::SignOptions;
use bdk_coin_select::metrics::LowestFee;
use bdk_coin_select::Candidate;
use bdk_coin_select::ChangePolicy;
use bdk_coin_select::CoinSelector;
use bdk_coin_select::DrainWeights;
use bdk_coin_select::Target;
use bitcoin::secp256k1::KeyPair;
use bitcoin::Network;
use bitcoin::TxIn;
use ln_dlc_storage::DlcStorageProvider;
use ln_dlc_storage::WalletStorage;
use std::sync::Arc;

const COIN_SELECTION_MAX_ROUNDS: usize = 100_000;

#[derive(Clone)]
pub struct DlcWallet<D, S, N> {
    on_chain_wallet: Arc<OnChainWallet<D>>,
    blockchain: Arc<Blockchain<N>>,
    dlc_storage: Arc<DlcStorageProvider<S>>,
}

impl<D, S, N> DlcWallet<D, S, N> {
    pub fn new(
        on_chain_wallet: Arc<OnChainWallet<D>>,
        dlc_storage: Arc<DlcStorageProvider<S>>,
        blockchain: Arc<Blockchain<N>>,
    ) -> Self {
        Self {
            on_chain_wallet,
            blockchain,
            dlc_storage,
        }
    }
}

impl<D, S, N> dlc_manager::Blockchain for DlcWallet<D, S, N>
where
    D: BdkStorage,
    N: Storage,
{
    fn send_transaction(
        &self,
        tx: &bitcoin_old::Transaction,
    ) -> Result<(), dlc_manager::error::Error> {
        let tx = to_tx_30(tx.clone());

        self.blockchain
            .broadcast_transaction_blocking(&tx)
            .map_err(|e| dlc_manager::error::Error::WalletError(format!("{e:#}").into()))?;

        Ok(())
    }

    fn get_network(
        &self,
    ) -> Result<bitcoin_old::network::constants::Network, dlc_manager::error::Error> {
        let network = self.on_chain_wallet.network;
        let network = match network {
            Network::Bitcoin => bitcoin_old::network::constants::Network::Bitcoin,
            Network::Testnet => bitcoin_old::network::constants::Network::Testnet,
            Network::Signet => bitcoin_old::network::constants::Network::Signet,
            Network::Regtest => bitcoin_old::network::constants::Network::Regtest,
            _ => {
                return Err(dlc_manager::error::Error::BlockchainError(format!(
                    "Network not supported: {network}",
                )));
            }
        };

        Ok(network)
    }

    fn get_blockchain_height(&self) -> Result<u64, dlc_manager::error::Error> {
        Ok(self.on_chain_wallet.get_tip() as u64)
    }

    fn get_block_at_height(
        &self,
        height: u64,
    ) -> Result<bitcoin_old::Block, dlc_manager::error::Error> {
        let block_hash = self.blockchain.get_block_hash(height).map_err(|e| {
            dlc_manager::error::Error::BlockchainError(format!(
                "Could not find block hash at height {height}: {e:#}"
            ))
        })?;

        let block = self
            .blockchain
            .get_block_by_hash(&block_hash)
            .map_err(|e| {
                dlc_manager::error::Error::BlockchainError(format!(
                    "Could not find block by hash {}: {e:#}",
                    block_hash
                ))
            })?;

        Ok(to_block_29(block))
    }

    fn get_transaction(
        &self,
        txid: &bitcoin_old::Txid,
    ) -> Result<bitcoin_old::Transaction, dlc_manager::error::Error> {
        let txid = to_txid_30(*txid);

        let tx = self.on_chain_wallet.get_transaction(&txid).ok_or_else(|| {
            dlc_manager::error::Error::BlockchainError(format!("Transaction {txid} not found"))
        })?;

        let tx = to_tx_29(tx);

        Ok(tx)
    }

    fn get_transaction_confirmations(
        &self,
        txid: &bitcoin_old::Txid,
    ) -> Result<u32, dlc_manager::error::Error> {
        let txid = to_txid_30(*txid);

        let confirmations = self
            .blockchain
            .get_transaction_confirmations(&txid)
            .map_err(|e| {
                dlc_manager::error::Error::BlockchainError(format!(
                    "Could not get confirmations for transaction {txid}: {e:#}",
                ))
            })?;

        Ok(confirmations)
    }

    fn get_txo_confirmations(
        &self,
        txo: &bitcoin_old::OutPoint,
    ) -> Result<Option<(u32, bitcoin_old::Txid)>, dlc_manager::error::Error> {
        let txo = to_outpoint_30(*txo);

        let confirmations = self.blockchain.get_txo_confirmations(&txo).map_err(|e| {
            dlc_manager::error::Error::BlockchainError(format!(
                "Could not get confirmations for txo {txo}: {e:#}",
            ))
        })?;

        Ok(confirmations.map(|(confirmations, txid)| (confirmations, to_txid_29(txid))))
    }
}

impl<D: BdkStorage, S: TenTenOneStorage, N> dlc_manager::Wallet for DlcWallet<D, S, N> {
    fn get_new_address(&self) -> Result<bitcoin_old::Address, dlc_manager::error::Error> {
        let address = self
            .on_chain_wallet
            .get_new_address()
            .map_err(|e| dlc_manager::error::Error::WalletError((format!("{e:#}")).into()))?;
        let address = to_address_29(address);

        Ok(address)
    }

    // TODO: Use the extended private key, a special derivation path and an incremental index to
    // generate the secret key.
    fn get_new_secret_key(&self) -> Result<secp256k1_zkp::SecretKey, dlc_manager::error::Error> {
        let kp = KeyPair::new(&self.on_chain_wallet.secp, &mut rand::thread_rng());
        let sk = kp.secret_key();

        self.dlc_storage
            .upsert_key_pair(&kp.public_key(), &sk)
            .map_err(|e| {
                dlc_manager::error::Error::StorageError(format!("Failed to upsert key pair: {e:#}"))
            })?;

        let sk =
            secp256k1_zkp::SecretKey::from_slice(&sk.secret_bytes()).expect("valid conversion");

        Ok(sk)
    }

    /// Get UTXOs to create a DLC or a DLC channel.
    fn get_utxos_for_amount(
        &self,
        amount: u64,
        fee_rate: Option<u64>,
        base_weight_wu: u64,
        lock_utxos: bool,
    ) -> Result<Vec<dlc_manager::Utxo>, dlc_manager::error::Error> {
        let network = self.on_chain_wallet.network();

        let fee_rate = fee_rate.expect("always set by rust-dlc");

        // Get temporarily reserved UTXOs from in-memory storage.
        let mut reserved_outpoints = self.on_chain_wallet.locked_utxos.lock();

        let utxos = self.on_chain_wallet.list_unspent();

        let utxos: Vec<&LocalOutput> = utxos
            .iter()
            .filter(|utxo| !reserved_outpoints.contains(&utxo.outpoint))
            .filter(|utxo| !utxo.is_spent)
            .collect();

        // Filter out reserved and spent UTXOs to prevent double-spending attempts.
        let candidates = utxos
            .iter()
            .map(|utxo| {
                let tx_in = TxIn {
                    previous_output: utxo.outpoint,
                    ..Default::default()
                };

                let segwit_weight = tx_in.segwit_weight();

                // The 10101 wallet always generates SegWit addresses.
                //
                // TODO: Rework this once we use Taproot.
                let is_witness_program = true;

                Candidate::new(utxo.txout.value, segwit_weight as u32, is_witness_program)
            })
            .collect::<Vec<_>>();

        let target = Target {
            feerate: bdk_coin_select::FeeRate::from_sat_per_vb(fee_rate as f32),
            min_fee: 0,
            value: amount,
        };

        let mut coin_selector = CoinSelector::new(&candidates, base_weight_wu as u32);

        let dust_limit = 0;
        let long_term_feerate = bdk_coin_select::FeeRate::default_min_relay_fee();

        let change_policy = ChangePolicy::min_value_and_waste(
            DrainWeights::default(),
            dust_limit,
            target.feerate,
            long_term_feerate,
        );

        let metric = LowestFee {
            target,
            long_term_feerate,
            change_policy,
        };

        coin_selector
            .run_bnb(metric, COIN_SELECTION_MAX_ROUNDS)
            .map_err(|e| dlc_manager::error::Error::WalletError((format!("{e:#}")).into()))?;

        debug_assert!(coin_selector.is_target_met(target));

        let indices = coin_selector.selected_indices();

        let mut selected_utxos: Vec<dlc_manager::Utxo> = Vec::with_capacity(indices.len());
        for index in indices {
            let utxo = &utxos[*index];

            let address = bitcoin_old::Address::from_script(
                &to_script_29(utxo.txout.script_pubkey.clone()),
                to_network_29(network),
            )
            .expect("to be a valid address");

            let outpoint = utxo.outpoint;
            let utxo = dlc_manager::Utxo {
                tx_out: to_txout_29(utxo.txout.clone()),
                outpoint: to_outpoint_29(outpoint),
                address,
                redeem_script: bitcoin_old::Script::new(),
                reserved: false,
            };

            if lock_utxos {
                // Add selected UTXOs to reserve to prevent future double-spend attempts.
                reserved_outpoints.push(outpoint);
            }

            selected_utxos.push(utxo);
        }

        Ok(selected_utxos)
    }

    fn import_address(
        &self,
        address: &bitcoin_old::Address,
    ) -> Result<(), dlc_manager::error::Error> {
        tracing::debug!(%address, "Choosing to ignore watching DLC-related address");

        Ok(())
    }

    fn unreserve_utxos(
        &self,
        outpoints: &[bitcoin_old::OutPoint],
    ) -> Result<(), dlc_manager::error::Error> {
        self.on_chain_wallet.unreserve_utxos(outpoints);

        Ok(())
    }
}

impl<D, S: TenTenOneStorage, N> dlc_manager::Signer for DlcWallet<D, S, N> {
    fn sign_psbt_input(
        &self,
        psbt: &mut bitcoin_old::psbt::PartiallySignedTransaction,
        _index: usize,
    ) -> Result<(), dlc_manager::error::Error> {
        let mut psbt_30 = to_psbt_30(psbt.clone());

        self.on_chain_wallet
            .sign_psbt(
                &mut psbt_30,
                SignOptions {
                    trust_witness_utxo: true,
                    ..Default::default()
                },
            )
            .map_err(|e| dlc_manager::error::Error::WalletError((format!("{e:#}")).into()))?;

        *psbt = to_psbt_29(psbt_30.clone());

        Ok(())
    }

    fn get_secret_key_for_pubkey(
        &self,
        pk: &secp256k1_zkp::PublicKey,
    ) -> Result<secp256k1_zkp::SecretKey, dlc_manager::error::Error> {
        let public_key =
            bitcoin::secp256k1::PublicKey::from_slice(&pk.serialize()).expect("valid conversion");
        let sk = self
            .dlc_storage
            .get_priv_key_for_pubkey(&public_key)
            .map_err(|e| {
                dlc_manager::error::Error::StorageError(format!("Failed to load SK: {e:#}"))
            })?
            .ok_or_else(|| dlc_manager::error::Error::StorageError("Unknown PK".to_string()))?;
        let sk =
            secp256k1_zkp::SecretKey::from_slice(&sk.secret_bytes()).expect("valid conversion");

        Ok(sk)
    }
}
