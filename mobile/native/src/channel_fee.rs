use crate::commons::reqwest_client;
use crate::config;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Txid;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::rust_dlc_manager::ChannelId;
use ln_dlc_node::node::ChannelManager;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::runtime::Handle;

#[derive(Clone)]
pub struct ChannelFeePaymentSubscriber {
    pub open_channel_info: Arc<Mutex<Option<(ChannelId, EsploraTransaction)>>>,
    pub channel_manager: Arc<ChannelManager>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EsploraTransaction {
    pub txid: String,
    pub fee: u32,
}

impl Subscriber for ChannelFeePaymentSubscriber {
    fn notify(&self, event: &EventInternal) {
        let result = match event {
            EventInternal::ChannelReady(channel_id) => self.register_channel_open_info(channel_id),
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
    pub fn new(channel_manager: Arc<ChannelManager>) -> Self {
        Self {
            open_channel_info: Arc::new(Mutex::new(None)),
            channel_manager,
        }
    }

    /// Attempts to pay the transaction fees for opening an inbound channel.
    fn pay_funding_transaction_fees(&self, amount_msats: u64) -> Result<()> {
        let (channel_id, transaction) = match self.get_open_channel_info() {
            Some((channel_id, transaction)) => (channel_id, transaction),
            None => {
                tracing::debug!("No pending funding transaction found!");
                return Ok(());
            }
        };

        let funding_tx_fees_msats = (transaction.fee * 1000) as u64;

        tokio::task::block_in_place(|| {
            tracing::debug!(
                channel_id = hex::encode(channel_id),
                funding_tx_fees_msats,
                "Waiting for outbound capacity on channel to pay jit channel opening fee.",
            );
            Handle::current()
                .block_on(self.wait_for_outbound_capacity(channel_id, funding_tx_fees_msats))
        })?;

        tracing::debug!(
            "Trying to pay channel opening fees of {} sats",
            transaction.fee
        );
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

        match ln_dlc::send_payment(&invoice_str) {
            Ok(_) => {
                // unset the funding transaction marking it as being paid.
                self.unset_open_channel_info();
                tracing::info!("Successfully triggered funding transaction fees payment of {funding_tx_fees_msats} msats to {}", config::get_coordinator_info().pubkey);
            }
            Err(e) => {
                tracing::error!("Failed to pay funding transaction fees of {funding_tx_fees_msats} msats to {}. Error: {e:#}", config::get_coordinator_info().pubkey);
            }
        };

        Ok(())
    }

    /// Register jit channel opening transaction for fee payment
    fn register_channel_open_info(&self, channel_id: &ChannelId) -> Result<()> {
        let channel_id_as_str = hex::encode(channel_id);
        tracing::debug!("Received new inbound channel with id {channel_id_as_str}");

        let txid = ln_dlc::get_funding_transaction(channel_id)?;

        let transaction: EsploraTransaction = tokio::task::block_in_place(|| {
            Handle::current().block_on(fetch_funding_transaction(txid))
        })?;
        tracing::debug!("Successfully fetched transaction fees of {} for new inbound channel with id {channel_id_as_str}", transaction.fee);
        self.set_open_channel_info(channel_id, transaction);
        Ok(())
    }

    fn set_open_channel_info(&self, channel_id: &ChannelId, transaction: EsploraTransaction) {
        *self
            .open_channel_info
            .lock()
            .expect("Mutex to not be poisoned") = Some((*channel_id, transaction));
    }

    fn unset_open_channel_info(&self) {
        *self
            .open_channel_info
            .lock()
            .expect("Mutex to not be poisoned") = None;
    }

    fn get_open_channel_info(&self) -> Option<(ChannelId, EsploraTransaction)> {
        self.open_channel_info
            .lock()
            .expect("Mutex to not be poisoned")
            .clone()
    }

    async fn wait_for_outbound_capacity(
        &self,
        channel_id: ChannelId,
        funding_tx_fees_msats: u64,
    ) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let channel_details = match self
                    .channel_manager
                    .get_channel_details(&channel_id) {
                    Some(channel_details) => channel_details,
                    None => {
                        bail!("Could not find channel details for {}", hex::encode(channel_id));
                    },
                };

                if channel_details.outbound_capacity_msat >= funding_tx_fees_msats {
                    tracing::debug!(channel_details.outbound_capacity_msat, channel_id=hex::encode(channel_id),
                        "Channel has enough outbound capacity");
                    return Ok(())
                } else {
                    tracing::debug!(channel_id = hex::encode(channel_id), outbound_capacity_msats = channel_details.outbound_capacity_msat, funding_tx_fees_msats,
                        "Channel does not have enough outbound capacity to pay jit channel opening fees yet. Waiting.");
                    tokio::time::sleep(Duration::from_millis(200)).await
                }
            }
        })
        .await?.map_err(|e| anyhow!("{e:#}"))
        .with_context(||format!(
            "Timed-out waiting for channel {} to become usable",
            hex::encode(channel_id)
        ))
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
