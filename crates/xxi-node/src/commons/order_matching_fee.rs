use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;

pub fn order_matching_fee(quantity: f32, price: Decimal, fee_per_cent: Decimal) -> bitcoin::Amount {
    let quantity = Decimal::from_f32(quantity).expect("quantity to fit in Decimal");

    let fee: f64 = match price != Decimal::ZERO {
        true => {
            let fee = quantity * (Decimal::ONE / price) * fee_per_cent;
            fee.round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                .to_f64()
                .expect("fee to fit in f64")
        }
        false => 0.0,
    };

    bitcoin::Amount::from_btc(fee).expect("fee to fit in bitcoin::Amount")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn calculate_order_matching_fee() {
        let price = Decimal::new(30209, 0);

        let fee = order_matching_fee(50.0, price, dec!(0.003));

        assert_eq!(fee.to_sat(), 497);
    }

    #[test]
    fn calculate_order_matching_fee_with_0() {
        let price = Decimal::new(0, 0);

        let fee = order_matching_fee(50.0, price, dec!(0.003));

        assert_eq!(fee.to_sat(), 0);
    }
}
