use bitcoin::Amount;
use futures_util::TryStreamExt;
use lnd_bridge::InvoiceState;
use lnd_bridge::LndBridge;
use tokio::sync::broadcast;
use xxi_node::commons;
use xxi_node::commons::Message;

/// Watches a hodl invoice with the given r_hash
pub fn spawn_invoice_watch(
    trader_sender: broadcast::Sender<Message>,
    lnd_bridge: LndBridge,
    invoice_params: commons::HodlInvoiceParams,
) {
    tokio::spawn(async move {
        let trader_pubkey = invoice_params.trader_pubkey;
        let r_hash = invoice_params.r_hash;
        tracing::info!(r_hash, "Subscribing to invoice updates");
        let mut stream = lnd_bridge.subscribe_to_invoice(r_hash.clone());

        loop {
            match stream.try_next().await {
                Ok(Some(invoice)) => match invoice.state {
                    InvoiceState::Open => {
                        tracing::debug!(%trader_pubkey, invoice.r_hash, "Watching hodl invoice.");
                        continue;
                    }
                    InvoiceState::Settled => {
                        tracing::info!(%trader_pubkey, invoice.r_hash, "Accepted hodl invoice has been settled.");
                        break;
                    }
                    InvoiceState::Canceled => {
                        tracing::warn!(%trader_pubkey, invoice.r_hash, "Pending hodl invoice has been canceled.");
                        break;
                    }
                    InvoiceState::Accepted => {
                        tracing::info!(%trader_pubkey, invoice.r_hash, "Pending hodl invoice has been accepted.");
                        if let Err(e) = trader_sender.send(Message::LnPaymentReceived {
                            r_hash: invoice.r_hash.clone(),
                            amount: Amount::from_sat(invoice.amt_paid_sat),
                        }) {
                            tracing::error!(%trader_pubkey, r_hash = invoice.r_hash, "Failed to send payment received event to app. Error: {e:#}")
                        }
                        continue;
                    }
                },
                Ok(None) => {
                    tracing::error!(%trader_pubkey, r_hash, "Websocket sender died.");
                    break;
                }
                Err(e) => {
                    tracing::error!(%trader_pubkey, r_hash, "Websocket closed the connection. Error: {e:#}");
                    break;
                }
            }
        }

        tracing::info!(%trader_pubkey, r_hash, "Stopping hodl invoice watch.");
    });
}
