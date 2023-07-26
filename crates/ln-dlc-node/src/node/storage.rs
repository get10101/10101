use crate::channel::Channel;
use crate::channel::ChannelState;
use crate::transaction::Transaction;
use crate::HTLCStatus;
use crate::MillisatAmount;
use crate::PaymentFlow;
use crate::PaymentInfo;
use anyhow::Result;
use lightning::chain::keysinterface::DelayedPaymentOutputDescriptor;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::chain::keysinterface::StaticPaymentOutputDescriptor;
use lightning::chain::transaction::OutPoint;
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
    // Payments

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

    // Channel

    /// Insert or update a channel
    fn upsert_channel(&self, channel: Channel) -> Result<()>;
    /// Get channel by `user_channel_id`
    fn get_channel(&self, user_channel_id: &str) -> Result<Option<Channel>>;
    /// Get all non pending channels.
    fn all_non_pending_channels(&self) -> Result<Vec<Channel>>;

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
    payments: Arc<Mutex<HashMap<PaymentHash, PaymentInfo>>>,
    spendable_outputs: Arc<Mutex<HashMap<OutPoint, SpendableOutputDescriptor>>>,
    channels: Arc<Mutex<HashMap<String, Channel>>>,
    transactions: Arc<Mutex<HashMap<String, Transaction>>>,
}

impl Storage for InMemoryStore {
    // Payments

    fn insert_payment(&self, payment_hash: PaymentHash, info: PaymentInfo) -> Result<()> {
        self.payments_lock().insert(payment_hash, info);

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
        let mut payments = self.payments_lock();
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
                        description: "".to_string(),
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
        let payments = self.payments_lock();
        let info = payments.get(payment_hash);

        let payment = info.map(|info| (*payment_hash, info.clone()));

        Ok(payment)
    }

    fn all_payments(&self) -> Result<Vec<(PaymentHash, PaymentInfo)>> {
        let payments = self.payments_lock();
        let payments = payments.iter().map(|(a, b)| (*a, b.clone())).collect();

        Ok(payments)
    }

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

        self.spendable_outputs_lock().insert(*outpoint, descriptor);

        Ok(())
    }

    fn get_spendable_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<SpendableOutputDescriptor>> {
        Ok(self.spendable_outputs_lock().get(outpoint).cloned())
    }

    fn delete_spendable_output(&self, outpoint: &OutPoint) -> Result<()> {
        self.spendable_outputs_lock().remove(outpoint);

        Ok(())
    }

    fn all_spendable_outputs(&self) -> Result<Vec<SpendableOutputDescriptor>> {
        Ok(self.spendable_outputs_lock().values().cloned().collect())
    }

    // Channels

    fn upsert_channel(&self, channel: Channel) -> Result<()> {
        let user_channel_id = channel.user_channel_id.to_string();
        self.channels_lock().insert(user_channel_id, channel);
        Ok(())
    }

    fn get_channel(&self, user_channel_id: &str) -> Result<Option<Channel>> {
        let channel = self.channels_lock().get(user_channel_id).cloned();
        Ok(channel)
    }

    fn all_non_pending_channels(&self) -> Result<Vec<Channel>> {
        Ok(self
            .channels_lock()
            .values()
            .filter(|c| c.channel_state != ChannelState::Pending && c.funding_txid.is_some())
            .cloned()
            .collect())
    }

    // Transaction

    fn upsert_transaction(&self, transaction: Transaction) -> Result<()> {
        let txid = transaction.txid.to_string();
        self.transactions_lock().insert(txid, transaction);
        Ok(())
    }

    fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>> {
        let transaction = self.transactions_lock().get(txid).cloned();
        Ok(transaction)
    }

    fn all_transactions_without_fees(&self) -> Result<Vec<Transaction>> {
        Ok(self
            .transactions_lock()
            .values()
            .filter(|t| t.fee == 0)
            .cloned()
            .collect())
    }
}

impl InMemoryStore {
    fn payments_lock(&self) -> MutexGuard<HashMap<PaymentHash, PaymentInfo>> {
        self.payments.lock().expect("Mutex to not be poisoned")
    }

    fn spendable_outputs_lock(&self) -> MutexGuard<HashMap<OutPoint, SpendableOutputDescriptor>> {
        self.spendable_outputs
            .lock()
            .expect("Mutex to not be poisoned")
    }

    fn channels_lock(&self) -> MutexGuard<HashMap<String, Channel>> {
        self.channels.lock().expect("Mutex to not be poisoned")
    }

    fn transactions_lock(&self) -> MutexGuard<HashMap<String, Transaction>> {
        self.transactions.lock().expect("Mutex to not be poisoned")
    }
}
