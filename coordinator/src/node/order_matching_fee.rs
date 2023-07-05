use crate::node::Node;
use anyhow::Result;
use coordinator_commons::TradeParams;
use lightning_invoice::Invoice;
use orderbook_commons::order_matching_fee_taker;
use orderbook_commons::FEE_INVOICE_DESCRIPTION_PREFIX_TAKER;

/// How long the fee invoice will last for.
const INVOICE_EXPIRY: u32 = 3600;

impl Node {
    pub async fn fee_invoice_taker(&self, trade_params: &TradeParams) -> Result<Invoice> {
        let order_id = trade_params.filled_with.order_id;
        let description = format!("{FEE_INVOICE_DESCRIPTION_PREFIX_TAKER}{order_id}");

        let fee = order_matching_fee_taker(
            trade_params.quantity,
            trade_params.average_execution_price(),
        )
        .to_sat();

        self.inner.create_invoice(fee, description, INVOICE_EXPIRY)
    }
}
