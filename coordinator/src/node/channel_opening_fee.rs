use crate::db;
use crate::node::Node;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::ThirtyTwoByteHash;
use lightning::ln::PaymentHash;
use lightning_invoice::Bolt11Invoice;
use ln_dlc_node::channel::JIT_FEE_INVOICE_DESCRIPTION_PREFIX;
use ln_dlc_node::PaymentInfo;

impl Node {
    pub async fn channel_opening_fee_invoice(
        &self,
        amount: u64,
        funding_txid: String,
        expiry: Option<u32>,
    ) -> Result<Bolt11Invoice> {
        let description = format!("{JIT_FEE_INVOICE_DESCRIPTION_PREFIX}{funding_txid}");
        let invoice = self
            .inner
            .create_invoice(amount, description, expiry.unwrap_or(180))?;
        let payment_hash = invoice.payment_hash().into_32();
        let payment_hash_hex = payment_hash.to_hex();

        // In case we run into an error here we still return the invoice to the user to collect the
        // payment and log an error on the coordinator This means that it can happen that we receive
        // a payment that we cannot associate if we run into an error here.
        tokio::task::spawn_blocking({
            let node = self.clone();
            let invoice = invoice.clone();
            move || {
                if let Err(e) = associate_channel_open_fee_payment_with_channel(
                    node,
                    payment_hash,
                    invoice.into(),
                    funding_txid.clone(),
                ) {
                    tracing::error!(%funding_txid, payment_hash=%payment_hash_hex, "Failed to associate open channel fee payment with channel: {e:#}");
                }
            }
        });

        Ok(invoice)
    }
}

fn associate_channel_open_fee_payment_with_channel(
    node: Node,
    payment_hash: [u8; 32],
    payment_info: PaymentInfo,
    funding_txid: String,
) -> Result<()> {
    let mut conn = node.pool.get().context("Failed to get connection")?;

    // Insert the payment into the database
    db::payments::insert((PaymentHash(payment_hash), payment_info), &mut conn)
        .context("Failed to insert channel opening payment into database")?;

    // Update the payment hash in the channels table. The channel is identified by the
    // funding_tx
    db::channels::update_payment_hash(PaymentHash(payment_hash), funding_txid, &mut conn)
        .context("Failed to update payment hash in channels table")?;

    Ok(())
}
