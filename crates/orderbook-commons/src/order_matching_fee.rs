use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;

/// The order-matching fee per cent for the taker.
const TAKER_FEE: (i64, u32) = (30, 4);

pub fn order_matching_fee_taker(quantity: f32, price: Decimal) -> bitcoin::Amount {
    order_matching_fee(quantity, price, Decimal::new(TAKER_FEE.0, TAKER_FEE.1))
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
