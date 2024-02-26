use crate::db;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use lightning::chain::transaction::OutPoint;
use lightning::sign::SpendableOutputDescriptor;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::node;
use ln_dlc_node::transaction::Transaction;

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

    // Channel

    fn upsert_channel(&self, channel: Channel) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::channels::upsert(channel.into(), &mut conn)
    }

    fn get_channel(&self, user_channel_id: &str) -> Result<Option<Channel>> {
        let mut conn = self.pool.get()?;
        let channel: Option<Channel> = db::channels::get(user_channel_id, &mut conn)
            .map_err(|e| anyhow!("{e:#}"))?
            .map(|c| c.into());
        Ok(channel)
    }

    fn all_non_pending_channels(&self) -> Result<Vec<Channel>> {
        let mut conn = self.pool.get()?;
        let channels = db::channels::get_all_non_pending_channels(&mut conn)?
            .into_iter()
            .map(|c| c.into())
            .collect::<Vec<_>>();

        Ok(channels)
    }

    fn get_announced_channel(&self, counterparty_pubkey: PublicKey) -> Result<Option<Channel>> {
        let mut conn = self.pool.get()?;
        let channel: Option<Channel> =
            db::channels::get_announced_channel(&counterparty_pubkey.to_string(), &mut conn)
                .map_err(|e| anyhow!("{e:#}"))?
                .map(|c| c.into());
        Ok(channel)
    }

    fn get_channel_by_payment_hash(&self, _payment_hash: String) -> Result<Option<Channel>> {
        // the payment hash is not stored on the coordinator side.
        unimplemented!()
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
