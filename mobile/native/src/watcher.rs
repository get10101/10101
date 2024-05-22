use crate::event;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::state;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct InvoiceWatcher {
    sender: mpsc::Sender<bool>,
}

impl Subscriber for InvoiceWatcher {
    fn notify(&self, _: &EventInternal) {
        tokio::spawn({
            let sender = self.sender.clone();
            async move {
                if let Err(e) = sender.send(true).await {
                    tracing::error!("Failed to send accepted invoice event. Error: {e:#}");
                }
            }
        });
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::LnPaymentReceived]
    }
}

pub(crate) async fn watch_lightning_payment() -> Result<()> {
    let (sender, mut receiver) = mpsc::channel::<bool>(1);
    event::subscribe(InvoiceWatcher { sender });

    receiver.recv().await;

    Ok(())
}

/// Watches for the funding address to receive the given amount.
pub(crate) async fn watch_funding_address(
    funding_address: Address,
    funding_amount: Amount,
) -> Result<()> {
    let node = state::get_node().clone();
    let bdk_node = node.inner.clone();

    loop {
        match bdk_node.get_unspent_txs(&funding_address).await {
            Ok(ref v) if v.is_empty() => {
                tracing::debug!(%funding_address, %funding_amount, "No tx found for address");
            }
            Ok(txs) => {
                // we sum up the total value in this output and check if it is big enough
                // for the order
                let total_unspent_amount_received = txs
                    .into_iter()
                    .map(|(_, amount)| amount.to_sat())
                    .sum::<u64>();

                if total_unspent_amount_received >= funding_amount.to_sat() {
                    tracing::info!(
                        amount = total_unspent_amount_received.to_string(),
                        address = funding_address.to_string(),
                        "Address has been funded enough"
                    );

                    return Ok(());
                }
                tracing::debug!(
                    amount = total_unspent_amount_received.to_string(),
                    address = funding_address.to_string(),
                    "Address has not enough funds yet"
                );
            }
            Err(err) => {
                tracing::error!("Could not get utxo for address {err:?}");
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
