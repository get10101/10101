use crate::HTLCStatus;
use crate::MillisatAmount;
use crate::PaymentFlow;
use crate::PaymentInfo;
use anyhow::Result;
use lightning::ln::PaymentHash;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use time::OffsetDateTime;

/// Interface which defines what a persister of Lightning payments should be able to do.
pub trait PaymentPersister {
    /// Add a new payment.
    fn insert(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()>;
    /// Add a new payment or update an existing one.
    fn merge(
        &self,
        payment_hash: &PaymentHash,
        flow: PaymentFlow,
        amt_msat: MillisatAmount,
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
    ) -> Result<()>;
    /// Get a payment based on its payment hash.
    ///
    /// # Returns
    ///
    /// A tuple of the form `(PaymentHash, PaymentInfo)` if the payment hash was found in the
    /// persister; `Ok(None)` if the payment hash was not found in the persister; an error if
    /// accessing the persister failed.
    fn get(&self, payment_hash: &PaymentHash) -> Result<Option<(PaymentHash, PaymentInfo)>>;
    /// Get all payments stored in the persister.
    fn all(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>>;
}

#[derive(Default, Clone)]
pub struct PaymentMap(Arc<Mutex<HashMap<PaymentHash, PaymentInfo>>>);

impl PaymentPersister for PaymentMap {
    fn insert(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()> {
        self.0.lock().unwrap().insert(payment_hash, info);

        Ok(())
    }

    fn merge(
        &self,
        payment_hash: &PaymentHash,
        flow: PaymentFlow,
        amt_msat: MillisatAmount,
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
    ) -> Result<()> {
        let mut payments = self.0.lock().unwrap();
        match payments.get_mut(payment_hash) {
            Some(mut payment) => {
                payment.status = htlc_status;

                if let amt_msat @ MillisatAmount(Some(_)) = amt_msat {
                    payment.amt_msat = amt_msat
                }

                if let Some(preimage) = preimage {
                    payment.preimage = Some(preimage);
                }

                if let Some(secret) = secret {
                    payment.secret = Some(secret);
                }
            }
            None => {
                payments.insert(
                    *payment_hash,
                    PaymentInfo {
                        preimage,
                        secret,
                        status: htlc_status,
                        amt_msat: MillisatAmount(None),
                        flow,
                        timestamp: OffsetDateTime::now_utc(),
                    },
                );
            }
        }

        Ok(())
    }

    fn get(&self, payment_hash: &PaymentHash) -> Result<Option<(PaymentHash, PaymentInfo)>> {
        let payments = self.0.lock().unwrap();
        let info = payments.get(payment_hash);

        let payment = info.map(|info| (*payment_hash, *info));

        Ok(payment)
    }

    fn all(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        let payments = self.0.lock().unwrap();
        let payments = payments.iter().map(|(a, b)| (*a, *b)).collect();

        Ok(payments)
    }
}
