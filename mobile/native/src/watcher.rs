use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::state;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use std::time::Duration;
use tokio::sync::broadcast::Sender;

#[derive(Clone)]
pub struct InvoiceWatcher {
    pub sender: Sender<String>,
}

impl Subscriber for InvoiceWatcher {
    fn notify(&self, event: &EventInternal) {
        let runtime = match state::get_or_create_tokio_runtime() {
            Ok(runtime) => runtime,
            Err(e) => {
                tracing::error!("Failed to get tokio runtime. Error: {e:#}");
                return;
            }
        };
        let r_hash = match event {
            EventInternal::LnPaymentReceived { r_hash } => r_hash,
            _ => return,
        };

        runtime.spawn({
            let r_hash = r_hash.clone();
            let sender = self.sender.clone();
            async move {
                if let Err(e) = sender.send(r_hash.clone()) {
                    tracing::error!(%r_hash, "Failed to send accepted invoice event. Error: {e:#}");
                }
            }
        });
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::LnPaymentReceived]
    }
}

pub(crate) async fn watch_lightning_payment(watched_r_hash: String) -> Result<()> {
    tracing::debug!(%watched_r_hash, "Watching for lightning payment.");

    let mut subscriber = state::get_ln_payment_watcher().subscribe();
    loop {
        match subscriber.recv().await {
            Ok(r_hash) => {
                if watched_r_hash.eq(&r_hash) {
                    tracing::debug!(%watched_r_hash, "Received a watched lightning payment event.");
                    return Ok(());
                }

                tracing::debug!(%r_hash, %watched_r_hash, "Received a lightning payment event for an unknown lightning invoice.");
            }
            Err(e) => {
                tracing::error!("Failed to receive lighting payment received event. Error: {e:#}");
                break;
            }
        }
    }

    tracing::debug!(%watched_r_hash, "Stopping lightning payment watch.");

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
