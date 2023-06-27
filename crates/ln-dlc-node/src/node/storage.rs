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
use std::sync::MutexGuard;
use time::OffsetDateTime;

/// Storage layer interface.
///
/// It exists so that consumers of [`crate::node::Node`] can define their own storage.
pub trait Storage {
    /// Add a new payment.
    fn insert_payment(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()>;
    /// Add a new payment or update an existing one.
    fn merge_payment(
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
    /// A tuple of the form `(PaymentHash, PaymentInfo)` if the payment hash was found in the store;
    /// `Ok(None)` if the payment hash was not found in the store; an error if accessing the
    /// store failed.
    fn get_payment(&self, payment_hash: &PaymentHash)
        -> Result<Option<(PaymentHash, PaymentInfo)>>;
    /// Get all payments stored in the store.
    fn all_payments(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>>;
}

#[derive(Default, Clone)]
pub struct InMemoryStore(Arc<Mutex<HashMap<PaymentHash, PaymentInfo>>>);

impl Storage for InMemoryStore {
    fn insert_payment(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()> {
        self.lock().insert(payment_hash, info);

        Ok(())
    }

    fn merge_payment(
        &self,
        payment_hash: &PaymentHash,
        flow: PaymentFlow,
        amt_msat: MillisatAmount,
        htlc_status: HTLCStatus,
        preimage: Option<PaymentPreimage>,
        secret: Option<PaymentSecret>,
    ) -> Result<()> {
        let mut payments = self.lock();
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

    fn get_payment(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<Option<(PaymentHash, PaymentInfo)>> {
        let payments = self.lock();
        let info = payments.get(payment_hash);

        let payment = info.map(|info| (*payment_hash, *info));

        Ok(payment)
    }

    fn all_payments(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        let payments = self.lock();
        let payments = payments.iter().map(|(a, b)| (*a, *b)).collect();

        Ok(payments)
    }
}

impl InMemoryStore {
    fn lock(&self) -> MutexGuard<HashMap<PaymentHash, PaymentInfo>> {
        self.0.lock().expect("Mutex to not be poisoned")
    }
}
