use crate::db;
use crate::node::Node;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::ThirtyTwoByteHash;
use coordinator_commons::TradeParams;
use lightning::ln::PaymentHash;
use lightning_invoice::Bolt11Invoice;
use ln_dlc_node::PaymentInfo;
use orderbook_commons::order_matching_fee_taker;
use orderbook_commons::FEE_INVOICE_DESCRIPTION_PREFIX_TAKER;

/// How long the fee invoice will last for.
const INVOICE_EXPIRY: u32 = 3600;

impl Node {
    pub async fn fee_invoice_taker(
        &self,
        trade_params: &TradeParams,
    ) -> Result<(PaymentHash, Bolt11Invoice)> {
        let order_id = trade_params.filled_with.order_id;
        let description = format!("{FEE_INVOICE_DESCRIPTION_PREFIX_TAKER}{order_id}");

        let fee = order_matching_fee_taker(
            trade_params.quantity,
            trade_params.average_execution_price(),
        )
        .to_sat();

        let invoice = self
            .inner
            .create_invoice(fee, description, INVOICE_EXPIRY)?;

        let fee_payment_hash = PaymentHash((*invoice.payment_hash()).into_32());
        let fee_payment_info = PaymentInfo::from(invoice.clone());
        let mut conn = self.pool.get()?;

        db::payments::insert((fee_payment_hash, fee_payment_info), &mut conn)
            .context("Failed to insert payment into database")?;

        Ok((fee_payment_hash, invoice))
    }
}
