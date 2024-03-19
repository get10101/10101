use crate::transaction::Transaction;
use anyhow::Result;
use lightning::chain::transaction::OutPoint;
use lightning::sign::DelayedPaymentOutputDescriptor;
use lightning::sign::SpendableOutputDescriptor;
use lightning::sign::StaticPaymentOutputDescriptor;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Storage layer interface.
///
/// It exists so that consumers of [`crate::node::Node`] can define their own storage.
pub trait Storage {
    // Spendable outputs

    /// Add a new [`SpendableOutputDescriptor`] to the store.
    fn insert_spendable_output(&self, descriptor: SpendableOutputDescriptor) -> Result<()>;
    /// Get a [`SpendableOutputDescriptor`] by its [`OutPoint`].
    ///
    /// # Returns
    ///
    /// A [`SpendableOutputDescriptor`] if the [`OutPoint`] hash was found in the store; `Ok(None)`
    /// if the [`OutPoint`] was not found in the store; an error if accessing the store failed.
    fn get_spendable_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<SpendableOutputDescriptor>>;

    /// Delete a [`SpendableOutputDescriptor`] by its [`OutPoint`].
    fn delete_spendable_output(&self, outpoint: &OutPoint) -> Result<()>;

    /// Get all [`SpendableOutputDescriptor`]s stored.
    fn all_spendable_outputs(&self) -> Result<Vec<SpendableOutputDescriptor>>;

    // Transaction

    /// Insert or update a transaction
    fn upsert_transaction(&self, transaction: Transaction) -> Result<()>;
    /// Get transaction by `txid`
    fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>>;
    /// Get all transactions without fees
    fn all_transactions_without_fees(&self) -> Result<Vec<Transaction>>;
}

#[derive(Default, Clone)]
pub struct InMemoryStore {
    spendable_outputs: Arc<Mutex<HashMap<OutPoint, SpendableOutputDescriptor>>>,
    transactions: Arc<Mutex<HashMap<String, Transaction>>>,
}

impl Storage for InMemoryStore {
    // Spendable outputs

    fn insert_spendable_output(&self, descriptor: SpendableOutputDescriptor) -> Result<()> {
        use SpendableOutputDescriptor::*;
        let outpoint = match &descriptor {
            // Static outputs don't need to be persisted because they pay directly to an address
            // owned by the on-chain wallet
            StaticOutput { .. } => return Ok(()),
            DelayedPaymentOutput(DelayedPaymentOutputDescriptor { outpoint, .. }) => outpoint,
            StaticPaymentOutput(StaticPaymentOutputDescriptor { outpoint, .. }) => outpoint,
        };

        self.spendable_outputs.lock().insert(*outpoint, descriptor);

        Ok(())
    }

    fn get_spendable_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<SpendableOutputDescriptor>> {
        Ok(self.spendable_outputs.lock().get(outpoint).cloned())
    }

    fn delete_spendable_output(&self, outpoint: &OutPoint) -> Result<()> {
        self.spendable_outputs.lock().remove(outpoint);

        Ok(())
    }

    fn all_spendable_outputs(&self) -> Result<Vec<SpendableOutputDescriptor>> {
        Ok(self.spendable_outputs.lock().values().cloned().collect())
    }

    // Transaction

    fn upsert_transaction(&self, transaction: Transaction) -> Result<()> {
        let txid = transaction.txid().to_string();
        self.transactions.lock().insert(txid, transaction);
        Ok(())
    }

    fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>> {
        let transaction = self.transactions.lock().get(txid).cloned();
        Ok(transaction)
    }

    fn all_transactions_without_fees(&self) -> Result<Vec<Transaction>> {
        Ok(self
            .transactions
            .lock()
            .values()
            .filter(|t| t.fee() == 0)
            .cloned()
            .collect())
    }
}
