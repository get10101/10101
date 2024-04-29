use crate::db;
use anyhow::anyhow;
use anyhow::Result;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use lightning::chain::transaction::OutPoint;
use lightning::sign::SpendableOutputDescriptor;
use xxi_node::node;
use xxi_node::transaction::Transaction;

#[derive(Clone)]
pub struct NodeStorage {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl NodeStorage {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pool }
    }
}

impl node::Storage for NodeStorage {
    // Spendable outputs

    fn insert_spendable_output(&self, output: SpendableOutputDescriptor) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::spendable_outputs::insert(&mut conn, output)?;

        Ok(())
    }

    fn get_spendable_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<SpendableOutputDescriptor>> {
        let mut conn = self.pool.get()?;
        db::spendable_outputs::get(&mut conn, outpoint)
    }

    fn delete_spendable_output(&self, outpoint: &OutPoint) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::spendable_outputs::delete(&mut conn, outpoint)
    }

    fn all_spendable_outputs(&self) -> Result<Vec<SpendableOutputDescriptor>> {
        let mut conn = self.pool.get()?;
        db::spendable_outputs::get_all(&mut conn)
    }

    // Transaction

    fn upsert_transaction(&self, transaction: Transaction) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::transactions::upsert(transaction.into(), &mut conn)
    }

    fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>> {
        let mut conn = self.pool.get()?;
        let transaction = db::transactions::get(txid, &mut conn)
            .map_err(|e| anyhow!("{e:#}"))?
            .map(|t| t.into());
        Ok(transaction)
    }

    fn all_transactions_without_fees(&self) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        let transactions = db::transactions::get_all_without_fees(&mut conn)?
            .into_iter()
            .map(|t| t.into())
            .collect::<Vec<_>>();
        Ok(transactions)
    }
}
