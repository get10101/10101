use crate::commons::reqwest_client;
use crate::config;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Txid;
use lightning_invoice::Invoice;
use ln_dlc_node::node::rust_dlc_manager::ChannelId;
use serde::Deserialize;
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::runtime::Handle;

#[derive(Clone)]
pub struct ChannelFeePaymentSubscriber {
    pub open_channel_tx: Arc<Mutex<Option<EsploraTransaction>>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EsploraTransaction {
    pub txid: String,
    pub fee: u32,
}

impl Subscriber for ChannelFeePaymentSubscriber {
    fn notify(&self, event: &EventInternal) {
        let result = match event {
            EventInternal::ChannelReady(channel_id) => {
                self.register_funding_transaction(channel_id)
            }
            EventInternal::PaymentClaimed(amount_msats) => {
                self.pay_funding_transaction_fees(*amount_msats)
            }
            _ => Ok(()),
        };

        if let Err(e) = result {
            tracing::error!("{e:#}");
        }
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::ChannelReady, EventType::PaymentClaimed]
    }
}

impl ChannelFeePaymentSubscriber {
    pub fn new() -> Self {
        Self {
            open_channel_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Attempts to pay the transaction fees for opening an inbound channel.
    fn pay_funding_transaction_fees(&self, amount_msats: u64) -> Result<()> {
        let transaction = match self.get_funding_transaction() {
            Some(transaction) => transaction,
            None => {
                tracing::debug!("No pending funding transaction found!");
                return Ok(());
            }
        };

        tracing::debug!("Trying to pay channel opening fees of {}", transaction.fee);
        let funding_tx_fees_msats = (transaction.fee * 1000) as u64;
        let funding_txid = transaction.txid;

        if funding_tx_fees_msats > amount_msats {
            tracing::warn!("Trying to pay fees with an amount smaller than the fees!")
        }

        let invoice_str = tokio::task::block_in_place(|| {
            Handle::current().block_on(fetch_funding_transaction_fee_invoice(
                transaction.fee,
                funding_txid,
            ))
        })?;

        let invoice = Invoice::from_str(&invoice_str).context("Could not parse Invoice string")?;
        let _payment_hash = invoice.payment_hash();

        match ln_dlc::send_payment(&invoice_str) {
            Ok(_) => {
                // unset the funding transaction marking it as being paid.
                self.unset_funding_transaction();
                tracing::info!("Successfully triggered funding transaction fees payment of {funding_tx_fees_msats} msats to {}", config::get_coordinator_info().pubkey);
            }
            Err(e) => {
                tracing::error!("Failed to pay funding transaction fees of {funding_tx_fees_msats} msats to {}. Error: {e:#}", config::get_coordinator_info().pubkey);
            }
        };

        Ok(())
    }

    /// Register jit channel opening transaction for fee payment
    fn register_funding_transaction(&self, channel_id: &ChannelId) -> Result<()> {
        let channel_id_as_str = hex::encode(channel_id);
        tracing::debug!("Received new inbound channel with id {channel_id_as_str}");

        let txid = ln_dlc::get_funding_transaction(channel_id)?;

        let transaction: EsploraTransaction = tokio::task::block_in_place(|| {
            Handle::current().block_on(fetch_funding_transaction(txid))
        })?;
        tracing::debug!("Successfully fetched transaction fees of {} for new inbound channel with id {channel_id_as_str}", transaction.fee);
        self.set_funding_transaction(transaction);
        Ok(())
    }

    fn set_funding_transaction(&self, transaction: EsploraTransaction) {
        *self
            .open_channel_tx
            .lock()
            .expect("Mutex to not be poisoned") = Some(transaction);
    }

    fn unset_funding_transaction(&self) {
        *self
            .open_channel_tx
            .lock()
            .expect("Mutex to not be poisoned") = None;
    }

    fn get_funding_transaction(&self) -> Option<EsploraTransaction> {
        self.open_channel_tx
            .lock()
            .expect("Mutex to not be poisoned")
            .clone()
    }
}

async fn fetch_funding_transaction(txid: Txid) -> Result<EsploraTransaction> {
    reqwest_client()
        .get(format!("{}tx/{txid}", config::get_esplora_endpoint()))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| anyhow!("Failed to fetch transaction: {txid} from esplora. Error: {e:?}"))
}

async fn fetch_funding_transaction_fee_invoice(
    funding_tx_fee: u32,
    funding_txid: String,
) -> Result<String> {
    reqwest_client()
        .get(format!(
            "http://{}/api/invoice/open_channel_fee?amount={}&channel_funding_txid={}",
            config::get_http_endpoint(),
            funding_tx_fee,
            funding_txid.as_str()
        ))
        .send()
        .await?
        .text()
        .await
        .map_err(|e| anyhow!("Failed to fetch invoice from coordinator. Error:{e:?}"))
}
