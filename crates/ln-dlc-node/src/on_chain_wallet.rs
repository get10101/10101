use crate::bitcoin_conversion::to_outpoint_29;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::node::Fee;
use crate::seed::WalletSeed;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use bdk::chain::indexed_tx_graph::Indexer;
use bdk::chain::local_chain::LocalChain;
use bdk::chain::tx_graph::CalculateFeeError;
use bdk::chain::tx_graph::CanonicalTx;
use bdk::chain::Append;
use bdk::chain::ChainPosition;
use bdk::chain::PersistBackend;
use bdk::psbt::PsbtUtils;
use bdk::KeychainKind;
use bdk::LocalOutput;
use bdk::SignOptions;
use bitcoin::psbt::PartiallySignedTransaction;
use bitcoin::secp256k1::All;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Network;
use bitcoin::OutPoint;
use bitcoin::ScriptBuf;
use bitcoin::SignedAmount;
use bitcoin::Transaction;
use bitcoin::TxOut;
use bitcoin::Txid;
use lightning::chain::chaininterface::ConfirmationTarget;
use parking_lot::Mutex;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use time::OffsetDateTime;

/// Taken from mempool.space
const AVG_SEGWIT_TX_WEIGHT_VB: usize = 140;

#[derive(Clone)]
pub struct OnChainWallet<D> {
    bdk: Arc<RwLock<bdk::Wallet<D>>>,
    /// These `OutPoint`s are unlocked when the `OnChainWallet` is rebuilt or when
    /// `unreserve_utxos` is called.
    pub(crate) locked_utxos: Arc<Mutex<Vec<OutPoint>>>,
    pub(crate) fee_rate_estimator: Arc<FeeRateEstimator>,
    pub(crate) network: Network,
    pub(crate) secp: Secp256k1<All>,
}

impl<D> OnChainWallet<D> {
    pub fn get_balance(&self) -> bdk::wallet::Balance {
        self.bdk.read().get_balance()
    }

    /// List all the transactions related to this wallet.
    pub fn get_on_chain_history(&self) -> Vec<TransactionDetails> {
        let bdk = self.bdk.read();

        let txs = bdk.transactions().filter(|tx| {
            let tx = tx.tx_node.tx;
            bdk.spk_index().is_tx_relevant(tx)
        });

        txs.map(|tx| {
            let (sent, received) = bdk.sent_and_received(&tx.tx_node);

            let confirmation_status = self.get_confirmation_status(&tx.tx_node.txid());

            let fee = bdk.calculate_fee(&tx.tx_node).map(Amount::from_sat);

            TransactionDetails {
                transaction: tx.tx_node.tx.clone(),
                sent: Amount::from_sat(sent),
                received: Amount::from_sat(received),
                fee,
                confirmation_status,
            }
        })
        .collect()
    }

    pub fn network(&self) -> Network {
        self.bdk.read().network()
    }

    pub(crate) fn list_unspent(&self) -> Vec<LocalOutput> {
        self.bdk.read().list_unspent().collect()
    }

    pub(crate) fn unreserve_utxos(&self, outpoints: &[bitcoin_old::OutPoint]) {
        self.locked_utxos
            .lock()
            .retain(|utxo| !outpoints.contains(&to_outpoint_29(*utxo)));
    }

    pub(crate) fn get_transaction(&self, txid: &Txid) -> Option<Transaction> {
        let bdk = self.bdk.read();

        bdk.get_tx(*txid).map(|tx| tx.tx_node.tx).cloned()
    }

    pub(crate) fn get_confirmation_status(&self, txid: &Txid) -> ConfirmationStatus {
        let bdk = self.bdk.read();

        let (confirmation_height, confirmation_time) = match bdk.get_tx(*txid) {
            Some(CanonicalTx {
                chain_position: ChainPosition::Confirmed(anchor),
                ..
            }) => (anchor.confirmation_height, anchor.confirmation_time),
            Some(CanonicalTx {
                chain_position: ChainPosition::Unconfirmed(last_seen),
                ..
            }) => {
                let last_seen =
                    OffsetDateTime::from_unix_timestamp(last_seen as i64).expect("valid timestamp");

                return ConfirmationStatus::Mempool { last_seen };
            }
            None => return ConfirmationStatus::Unknown,
        };

        let tip = self.get_tip();
        let n_confirmations = match tip.checked_sub(confirmation_height) {
            Some(diff) => NonZeroU32::new(diff).unwrap_or({
                // Being included in a block counts as a confirmation!
                NonZeroU32::new(1).expect("non-zero value")
            }),
            None => {
                // The transaction shouldn't be ahead of the tip!
                debug_assert!(false);
                return ConfirmationStatus::Unknown;
            }
        };

        let timestamp =
            OffsetDateTime::from_unix_timestamp(confirmation_time as i64).expect("valid timestamp");

        ConfirmationStatus::Confirmed {
            n_confirmations,
            timestamp,
        }
    }

    pub(crate) fn get_tip(&self) -> u32 {
        self.bdk.read().local_chain().tip().block_id().height
    }

    /// Similar to `list_unspent`, but more types of UTXO are included here.
    pub(crate) fn get_utxos(&self) -> Vec<(OutPoint, TxOut)> {
        let bdk = self.bdk.read();

        bdk.tx_graph()
            .all_txouts()
            .map(|(outpoint, txout)| (outpoint, txout.clone()))
            .collect()
    }

    pub(crate) fn is_mine(&self, script_pubkey: &ScriptBuf) -> bool {
        self.bdk.read().is_mine(script_pubkey)
    }

    pub(crate) fn calculate_fee(
        &self,
        transaction: &Transaction,
    ) -> Result<u64, CalculateFeeError> {
        self.bdk.read().calculate_fee(transaction)
    }

    pub(crate) fn sign_psbt(
        &self,
        psbt: &mut PartiallySignedTransaction,
        sign_options: SignOptions,
    ) -> Result<()> {
        self.bdk.read().sign(psbt, sign_options)?;

        Ok(())
    }

    pub(crate) fn all_script_pubkeys(
        &self,
    ) -> BTreeMap<KeychainKind, impl Iterator<Item = (u32, ScriptBuf)> + Clone> {
        self.bdk.read().all_unbounded_spk_iters()
    }

    pub(crate) fn local_chain(&self) -> LocalChain {
        self.bdk.read().local_chain().clone()
    }

    pub(crate) fn pre_sync_state(&self) -> (LocalChain, Vec<ScriptBuf>, Vec<Txid>, Vec<OutPoint>) {
        let bdk = self.bdk.read();

        let local_chain = bdk.local_chain().clone();

        // We must watch every new address we generate (until it is used).
        let unused_revealed_script_pubkeys = bdk
            .spk_index()
            .unused_spks()
            .map(|(_, _, s)| ScriptBuf::from(s))
            .collect();

        let unconfirmed_txids = bdk
            .tx_graph()
            .list_chain_txs(&local_chain, local_chain.tip().block_id())
            .filter(|tx| !tx.chain_position.is_confirmed())
            .map(|tx| tx.tx_node.txid)
            .collect();

        // We must watch every UTXO we own (until it is spent).
        let utxos = bdk
            .tx_graph()
            .all_txouts()
            .map(|(outpoint, _)| outpoint)
            .collect();

        (
            local_chain,
            unused_revealed_script_pubkeys,
            unconfirmed_txids,
            utxos,
        )
    }
}

impl<D> OnChainWallet<D>
where
    D: BdkStorage,
{
    pub fn new(
        network: Network,
        seed: WalletSeed,
        db: D,
        fee_rate_estimator: Arc<FeeRateEstimator>,
    ) -> Result<Self> {
        let secp = Secp256k1::new();

        tracing::info!(?network, "Creating on-chain wallet");

        let ext_priv_key = seed.derive_extended_priv_key(network)?;

        let bdk = bdk::Wallet::new_or_load(
            bdk::template::Bip84(ext_priv_key, KeychainKind::External),
            Some(bdk::template::Bip84(ext_priv_key, KeychainKind::Internal)),
            db,
            network,
        )
        .map_err(|e| anyhow!("{e:?}"))?;
        let bdk = RwLock::new(bdk);
        let bdk = Arc::new(bdk);

        Ok(Self {
            bdk,
            locked_utxos: Default::default(),
            fee_rate_estimator,
            network,
            secp,
        })
    }

    pub fn get_new_address(&self) -> Result<Address> {
        let address = self
            .bdk
            .write()
            .try_get_address(bdk::wallet::AddressIndex::New)
            .map_err(|e| anyhow!("{e:?}"))?;

        Ok(address.address)
    }

    pub fn get_unused_address(&self) -> Result<Address> {
        let address = self
            .bdk
            .write()
            .try_get_address(bdk::wallet::AddressIndex::LastUnused)
            .map_err(|e| anyhow!("{e:?}"))?;

        Ok(address.address)
    }

    /// Send funds to the given address.
    ///
    /// If `amount_sat_or_drain` is `0` the wallet will be drained, i.e., all available funds
    /// will be spent.
    pub(crate) fn build_on_chain_payment_tx(
        &self,
        recipient: &Address,
        amount_sat_or_drain: u64,
        fee: Fee,
    ) -> Result<Transaction> {
        let tx = self
            .build_and_sign_psbt(recipient, amount_sat_or_drain, fee)?
            .extract_tx();

        let input_utxos = tx
            .input
            .iter()
            .map(|input| input.previous_output)
            .collect::<Vec<_>>();

        self.locked_utxos.lock().extend(input_utxos);

        let txid = tx.txid();

        let txo = tx
            .output
            .iter()
            .find(|txo| txo.script_pubkey == recipient.script_pubkey())
            .expect("transaction to have recipient TXO");
        let amount = Amount::from_sat(txo.value);

        tracing::info!(%txid, %amount, %recipient, "Built on-chain payment transaction");

        Ok(tx)
    }

    /// Build a PSBT to send some sats to an [`Address`].
    pub fn build_psbt(
        &self,
        recipient: &Address,
        amount_sat_or_drain: u64,
        fee: Fee,
    ) -> Result<PartiallySignedTransaction> {
        let script_pubkey = recipient.script_pubkey();

        let wallet = &mut self.bdk.write();
        let mut builder = wallet.build_tx();

        let locked_utxos = self.locked_utxos.lock();
        for outpoint in locked_utxos.iter() {
            builder.add_unspendable(*outpoint);
        }

        if amount_sat_or_drain > 0 {
            builder.add_recipient(script_pubkey, amount_sat_or_drain);
        } else {
            builder.drain_wallet().drain_to(script_pubkey);
        }

        let fee_rate = match fee {
            Fee::Priority(target) => self.fee_rate_estimator.get(target),
            Fee::FeeRate(fee_rate) => fee_rate,
        };

        builder.fee_rate(fee_rate);

        let psbt = builder.finish().map_err(|e| anyhow!("{e:?}"))?;

        Ok(psbt)
    }

    pub fn build_and_sign_psbt(
        &self,
        recipient: &Address,
        amount_sat_or_drain: u64,
        fee: Fee,
    ) -> Result<PartiallySignedTransaction> {
        let mut psbt = self.build_psbt(recipient, amount_sat_or_drain, fee)?;

        let finalized = self
            .bdk
            .write()
            .sign(&mut psbt, SignOptions::default())
            .map_err(|e| anyhow!("{e:?}"))?;

        if !finalized {
            bail!("PSBT not finalized");
        }

        Ok(psbt)
    }

    /// Estimate the fee for sending funds to a given [`Address`].
    pub fn estimate_fee(
        &self,
        recipient: &Address,
        amount_sat_or_drain: u64,
        confirmation_target: ConfirmationTarget,
    ) -> Result<Amount> {
        let psbt = self.build_psbt(
            recipient,
            amount_sat_or_drain,
            Fee::Priority(confirmation_target),
        )?;

        let fee_sat = match psbt.fee_amount() {
            Some(fee) => fee,
            None => {
                let rate = self.fee_rate_estimator.get(confirmation_target);
                rate.fee_vb(AVG_SEGWIT_TX_WEIGHT_VB)
            }
        };

        Ok(Amount::from_sat(fee_sat))
    }

    pub(crate) fn commit_wallet_update(&self, update: bdk::wallet::Update) -> Result<()> {
        let mut bdk = self.bdk.write();

        bdk.apply_update(update)?;

        bdk.commit().map_err(|e| anyhow!("{e:?}"))?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct TransactionDetails {
    pub transaction: Transaction,
    pub sent: Amount,
    pub received: Amount,
    // The fee is the only part of this struct that we might fail to compute. We forward the error
    // so that consumers can decide how to proceed.
    pub fee: Result<Amount, CalculateFeeError>,
    pub confirmation_status: ConfirmationStatus,
}

impl TransactionDetails {
    pub fn net_amount(&self) -> Result<SignedAmount> {
        let received = self.received.to_signed()?;
        let sent = self.sent.to_signed()?;

        Ok(received - sent)
    }
}

#[derive(Debug)]
pub enum ConfirmationStatus {
    Unknown,
    Mempool {
        last_seen: OffsetDateTime,
    },
    Confirmed {
        n_confirmations: NonZeroU32,
        timestamp: OffsetDateTime,
    },
}

impl ConfirmationStatus {
    pub fn n_confirmations(&self) -> u32 {
        match self {
            ConfirmationStatus::Confirmed {
                n_confirmations, ..
            } => (*n_confirmations).into(),
            ConfirmationStatus::Unknown | ConfirmationStatus::Mempool { .. } => 0,
        }
    }
}

pub trait BdkStorage: PersistBackend<bdk::wallet::ChangeSet> + Send + Sync + 'static {}

#[derive(Default)]
pub struct InMemoryStorage(Option<bdk::wallet::ChangeSet>);

impl InMemoryStorage {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T> BdkStorage for T where T: PersistBackend<bdk::wallet::ChangeSet> + Send + Sync + 'static {}

impl PersistBackend<bdk::wallet::ChangeSet> for InMemoryStorage {
    type WriteError = anyhow::Error;
    type LoadError = anyhow::Error;

    fn write_changes(
        &mut self,
        changeset: &bdk::wallet::ChangeSet,
    ) -> Result<(), Self::WriteError> {
        if changeset.is_empty() {
            return Ok(());
        }

        let original = self.0.get_or_insert(changeset.clone());

        original.append(changeset.clone());

        Ok(())
    }

    fn load_from_persistence(&mut self) -> Result<Option<bdk::wallet::ChangeSet>, Self::LoadError> {
        Ok(self.0.clone())
    }
}
