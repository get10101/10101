use anyhow::Result;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::chain::transaction::OutPoint;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use ln_dlc_node::node;
use ln_dlc_node::HTLCStatus;
use ln_dlc_node::MillisatAmount;
use ln_dlc_node::PaymentFlow;
use ln_dlc_node::PaymentInfo;

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

    fn insert_payment(&self, _payment_hash: PaymentHash, _info: PaymentInfo) -> Result<()> {
        todo!()
    }
    fn merge_payment(
        &self,
        _payment_hash: &PaymentHash,
        _flow: PaymentFlow,
        _amt_msat: MillisatAmount,
        _htlc_status: HTLCStatus,
        _preimage: Option<PaymentPreimage>,
        _secret: Option<PaymentSecret>,
    ) -> Result<()> {
        todo!()
    }
    fn get_payment(
        &self,
        _payment_hash: &PaymentHash,
    ) -> Result<Option<(PaymentHash, PaymentInfo)>> {
        todo!()
    }
    fn all_payments(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        todo!()
    }

    // Spendable outputs

    fn insert_spendable_output(&self, output: SpendableOutputDescriptor) -> Result<()> {
        let mut conn = self.pool.get()?;
        crate::db::spendable_outputs::insert(&mut conn, output)?;

        Ok(())
    }

    fn get_spendable_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<SpendableOutputDescriptor>> {
        let mut conn = self.pool.get()?;
        crate::db::spendable_outputs::get(&mut conn, outpoint)
    }

    fn all_spendable_outputs(&self) -> Result<Vec<SpendableOutputDescriptor>> {
        let mut conn = self.pool.get()?;
        crate::db::spendable_outputs::get_all(&mut conn)
    }
}
