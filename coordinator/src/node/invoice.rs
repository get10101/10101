use futures_util::TryStreamExt;
use lnd_bridge::InvoiceState;
use lnd_bridge::LndBridge;
use xxi_node::commons;

/// Watches a hodl invoice with the given r_hash
pub fn spawn_invoice_watch(lnd_bridge: LndBridge, invoice_params: commons::HodlInvoiceParams) {
    tokio::spawn(async move {
        let trader_pubkey = invoice_params.trader_pubkey;
        let r_hash = invoice_params.r_hash;
        let mut stream = lnd_bridge.subscribe_to_invoice(r_hash.clone());

        'watch_invoice: loop {
            match stream.try_next().await {
                Ok(Some(invoice)) => {
                    match invoice.state {
                        InvoiceState::Open => {
                            tracing::debug!(%trader_pubkey, r_hash, "Watching hodl invoice.");
                            continue 'watch_invoice;
                        }
                        InvoiceState::Settled => {
                            tracing::info!(%trader_pubkey, r_hash, "Accepted hodl invoice has been settled.");
                            break 'watch_invoice;
                        }
                        InvoiceState::Canceled => {
                            tracing::warn!(%trader_pubkey, r_hash, "Pending hodl invoice has been canceled.");
                            break 'watch_invoice;
                        }
                        InvoiceState::Accepted => {
                            tracing::info!(%trader_pubkey, r_hash, "Pending hodl invoice has been accepted.");
                            // TODO(holzeis): Notify the client about the accepted invoice.
                            // wait for the invoice to get settled.
                            continue 'watch_invoice;
                        }
                    }
                }
                Ok(None) => {
                    tracing::error!(%trader_pubkey, r_hash, "Websocket sender died.");
                    break 'watch_invoice;
                }
                Err(e) => {
                    tracing::error!(%trader_pubkey, r_hash, "Websocket closed the connection. Error: {e:#}");
                    break 'watch_invoice;
                }
            }
        }

        tracing::info!(%trader_pubkey, r_hash, "Stopping hodl invoice watch.");
    });
}
