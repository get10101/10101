use crate::db;
use crate::db::payments;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use lightning::chain::transaction::OutPoint;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning::sign::SpendableOutputDescriptor;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::node;
use ln_dlc_node::transaction::Transaction;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::MillisatAmount;
use ln_dlc_node::PaymentFlow;
use ln_dlc_node::PaymentInfo;
use time::OffsetDateTime;

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
    // Payments

    fn insert_payment(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()> {
        let mut conn = self.pool.get()?;
        payments::insert((payment_hash, info), &mut conn)
    }

    fn merge_payment(
        &self,
        payment_hash: &PaymentHash,
        flow: PaymentFlow,
        amt_msat: MillisatAmount,
        fee_msat: MillisatAmount,
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;

        match payments::get(*payment_hash, &mut conn)? {
            Some(_) => {
                payments::update(
                    *payment_hash,
                    htlc_status,
                    amt_msat,
                    fee_msat,
                    preimage,
                    secret,
                    &mut conn,
                )?;
            }
            None => {
                payments::insert(
                    (
                        *payment_hash,
                        PaymentInfo {
                            preimage,
                            secret,
                            status: htlc_status,
                            amt_msat,
                            fee_msat,
                            flow,
                            timestamp: OffsetDateTime::now_utc(),
                            description: "".to_string(),
                            invoice: None,
                        },
                    ),
                    &mut conn,
                )?;
            }
        }

        Ok(())
    }

    fn get_payment(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<Option<(PaymentHash, PaymentInfo)>> {
        let mut conn = self.pool.get()?;
        payments::get(*payment_hash, &mut conn)
    }

    fn all_payments(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        let mut conn = self.pool.get()?;
        payments::get_all(&mut conn)
    }

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
