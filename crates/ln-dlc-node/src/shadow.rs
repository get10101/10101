use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::on_chain_wallet::OnChainWallet;
use anyhow::Result;
use bdk::chain::tx_graph::CalculateFeeError;
use std::sync::Arc;

pub struct Shadow<D: BdkStorage, N: Storage> {
    storage: Arc<N>,
    wallet: Arc<OnChainWallet<D>>,
}

impl<D: BdkStorage, N: Storage> Shadow<D, N> {
    pub fn new(storage: Arc<N>, wallet: Arc<OnChainWallet<D>>) -> Self {
        Shadow { storage, wallet }
    }

    pub fn sync_transactions(&self) -> Result<()> {
        let transactions = self.storage.all_transactions_without_fees()?;
        tracing::debug!("Syncing {} shadow transactions", transactions.len());

        let wallet = self.wallet.clone();

        for transaction in transactions.iter() {
            let txid = transaction.txid();

            match wallet.get_transaction(&txid) {
                Some(tx) => match wallet.calculate_fee(&tx) {
                    Ok(fee) => {
                        self.storage
                            .upsert_transaction(transaction.clone().with_fee(fee))?;
                    }
                    Err(e @ CalculateFeeError::NegativeFee(_)) => {
                        tracing::error!(%txid, "Failed to get fee: {e}");
                    }
                    Err(e @ CalculateFeeError::MissingTxOut(_)) => {
                        tracing::warn!(%txid, "Failed to get fee: {e}");
                        // TODO: We should consider calling `insert_txout` to add all the `TxOut`s
                        // that we don't own so that BDK can actually calculate the fee. Of course,
                        // the fee will be shared with other wallets if we don't own all the
                        // transaction inputs, and BDK won't be able to decide on the split.
                    }
                },
                None => {
                    tracing::warn!(%txid, "Failed to get transaction details");
                }
            };
        }
        Ok(())
    }
}
