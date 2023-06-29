use crate::node::Node;
use anyhow::Result;
use coordinator_commons::TradeParams;
use lightning_invoice::Invoice;
use orderbook_commons::FEE_INVOICE_DESCRIPTION_PREFIX_TAKER;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;

/// The order-matching fee per cent for the taker.
const TAKER_FEE: (i64, u32) = (30, 4);

/// How long the fee invoice will last for.
const INVOICE_EXPIRY: u32 = 3600;

impl Node {
    pub async fn fee_invoice_taker(&self, trade_params: &TradeParams) -> Result<Invoice> {
        let order_id = trade_params.filled_with.order_id;
        let description = format!("{FEE_INVOICE_DESCRIPTION_PREFIX_TAKER}{order_id}");

        let fee = order_matching_fee(
            trade_params.quantity,
            trade_params.average_execution_price(),
            Decimal::new(TAKER_FEE.0, TAKER_FEE.1),
        )
        .to_sat();

        self.inner.create_invoice(fee, description, INVOICE_EXPIRY)
    }
}

fn order_matching_fee(quantity: f32, price: Decimal, fee_per_cent: Decimal) -> bitcoin::Amount {
    let quantity = Decimal::from_f32(quantity).expect("quantity to fit in Decimal");
    let price = price;

    let fee = quantity * (Decimal::ONE / price) * fee_per_cent;
    let fee = fee
        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
        .to_f64()
        .expect("fee to fit in f64");

    bitcoin::Amount::from_btc(fee).expect("fee to fit in bitcoin::Amount")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_order_matching_fee() {
        let price = Decimal::new(30209, 0);

        let fee = order_matching_fee(50.0, price, Decimal::new(TAKER_FEE.0, TAKER_FEE.1));

        assert_eq!(fee.to_sat(), 497);
    }
}
