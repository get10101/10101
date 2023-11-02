use crate::Direction;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin;
use bdk::bitcoin::Denomination;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::ops::Neg;

pub const BTCUSD_MAX_PRICE: u64 = 1_048_575;

/// Calculate the collateral in BTC.
pub fn calculate_margin(open_price: Decimal, quantity: f32, leverage: f32) -> u64 {
    let quantity = Decimal::try_from(quantity).expect("quantity to fit into decimal");
    let leverage = Decimal::try_from(leverage).expect("leverage to fix into decimal");

    if open_price == Decimal::ZERO || leverage == Decimal::ZERO {
        // just to avoid div by 0 errors
        return 0;
    }

    let margin = quantity / (open_price * leverage);

    // TODO: Shift the decimal without going into float
    let margin =
        margin.round_dp_with_strategy(8, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    let margin = margin.to_f64().expect("collateral to fit into f64");

    bitcoin::Amount::from_btc(margin)
        .expect("collateral to fit in amount")
        .to_sat()
}

/// Calculate the quantity from price, collateral and leverage
/// Margin in sats, calculation in BTC
pub fn calculate_quantity(opening_price: f32, margin: u64, leverage: f32) -> f32 {
    let margin_amount = bitcoin::Amount::from_sat(margin);

    let margin = Decimal::try_from(margin_amount.to_float_in(Denomination::Bitcoin))
        .expect("collateral to fit into decimal");
    let open_price = Decimal::try_from(opening_price).expect("price to fit into decimal");
    let leverage = Decimal::try_from(leverage).expect("leverage to fit into decimal");

    let quantity = margin * open_price * leverage;
    quantity.to_f32().expect("quantity to fit into f32")
}

pub fn calculate_long_liquidation_price(leverage: Decimal, price: Decimal) -> Decimal {
    price * leverage / (leverage + Decimal::ONE)
}

/// Calculate liquidation price for the party going short.
pub fn calculate_short_liquidation_price(leverage: Decimal, price: Decimal) -> Decimal {
    // If the leverage is equal to 1, the liquidation price will go towards infinity
    if leverage == Decimal::ONE {
        return Decimal::from(BTCUSD_MAX_PRICE);
    }

    price * leverage / (leverage - Decimal::ONE)
}

/// Compute the payout for the given CFD parameters at a particular `closing_price`.
///
/// The `opening_price` of the position is the weighted opening price per quantity.
/// The `opening_price` is aggregated from all the execution prices of the orders that filled the
/// position; weighted by quantity. The closing price is the best bid/ask according to the orderbook
/// at a certain time.
///
/// Both leverages are supplied so that the total margin can be calculated and the PnL is capped by
/// the total margin available.
pub fn calculate_pnl(
    opening_price: Decimal,
    closing_price: Decimal,
    quantity: f32,
    long_leverage: f32,
    short_leverage: f32,
    direction: Direction,
) -> Result<i64> {
    let long_margin = calculate_margin(opening_price, quantity, long_leverage);
    let short_margin = calculate_margin(opening_price, quantity, short_leverage);

    let uncapped_pnl_long = {
        let quantity = Decimal::try_from(quantity).expect("quantity to fit into decimal");

        let uncapped_pnl = match opening_price != Decimal::ZERO && closing_price != Decimal::ZERO {
            true => (quantity / opening_price) - (quantity / closing_price),
            false => dec!(0.0),
        };

        let uncapped_pnl = uncapped_pnl * dec!(100_000_000);
        // we need to round to zero or else we might lose some sats somewhere
        uncapped_pnl.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointTowardZero)
    };

    let short_margin = Decimal::from_u64(short_margin).context("be able to parse u64")?;
    let long_margin = Decimal::from_u64(long_margin).context("to be abble to parse u64")?;

    let pnl = match direction {
        Direction::Long => {
            let max_win = uncapped_pnl_long.min(short_margin);
            if max_win.is_sign_negative() {
                max_win.max(long_margin.neg())
            } else {
                max_win
            }
        }

        Direction::Short => {
            let max_win = uncapped_pnl_long.neg().min(long_margin);
            if max_win.is_sign_negative() {
                max_win.max(short_margin.neg())
            } else {
                max_win
            }
        }
    };

    pnl.to_i64().context("to be able to convert into i64")
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn given_position_when_price_same_then_zero_pnl() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(20000);
        let quantity = 1.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();
        let pnl_short = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        assert_eq!(pnl_long, 0);
        assert_eq!(pnl_short, 0);
    }

    #[test]
    fn given_long_position_when_price_doubles_then_we_get_double() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(40000);
        let quantity = 100.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();

        assert_eq!(pnl_long, 250000);
    }

    #[test]
    fn given_long_position_when_price_halfs_then_we_loose_all() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(10000);
        let quantity = 100.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();

        // This is a liquidation, our margin is consumed by the loss
        assert_eq!(pnl_long, -250000);
    }

    #[test]
    fn given_short_position_when_price_doubles_then_we_loose_all() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(40000);
        let quantity = 100.0;
        let long_leverage = 1.0;
        let short_leverage = 2.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        assert_eq!(pnl_long, -250000);
    }

    #[test]
    fn given_short_position_when_price_halfs_then_we_get_double() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(10000);
        let quantity = 100.0;
        let long_leverage = 1.0;
        let short_leverage = 2.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        // This is a liquidation, our margin is consumed by the loss
        assert_eq!(pnl_long, 500000);
    }

    #[test]
    fn given_long_position_when_price_10_pc_up_then_18pc_profit() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(22000);
        let quantity = 20000.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();

        // Value taken from our CFD hedging model sheet
        assert_eq!(pnl_long, 9_090_909);
    }

    #[test]
    fn given_short_position_when_price_10_pc_up_then_18pc_loss() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(22000);
        let quantity = 20000.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        // Value taken from our CFD hedging model sheet
        assert_eq!(pnl_long, -9_090_909);
    }

    #[test]
    fn given_long_position_when_price_10_pc_down_then_22pc_loss() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(18000);
        let quantity = 20000.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();

        // Value taken from our CFD hedging model sheet
        assert_eq!(pnl_long, -11_111_111);
    }

    #[test]
    fn given_short_position_when_price_10_pc_down_then_22pc_profit() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(18000);
        let quantity = 20000.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        // Value taken from our CFD hedging model sheet
        assert_eq!(pnl_long, 11_111_111);
    }

    #[test]
    fn given_short_position_when_price_0() {
        let opening_price = Decimal::from(20000);
        let closing_price = Decimal::from(0);
        let quantity = 20000.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        // Value taken from our CFD hedging model sheet
        assert_eq!(pnl_long, 0);
    }

    #[test]
    fn given_uneven_price_should_round_down() {
        let opening_price = Decimal::from(1000);
        let closing_price = Decimal::from(1234);
        let quantity = 10.0;
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();

        // --> pnl should be ==> quantity / ((1/opening_price)-(1/closing_price))
        // should be 189,627.23 Sats , or 189,628 Sats away from zero
        assert_eq!(pnl_long, 189_627);
    }

    #[test]
    fn pnl_example_calculation() {
        let opening_price = Decimal::from(30_000);
        let closing_price = Decimal::from(20_002);
        let quantity = 60_000.0;
        let long_leverage = 2.0;
        let short_leverage = 2.0;

        let pnl_long = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        // --> pnl should be ==> quantity / ((1/opening_price)-(1/closing_price))
        // should be 0.99970003	BTC or 99970003 Sats
        assert_eq!(pnl_long, 99_970_003);
    }

    #[test]
    fn assert_to_not_lose_more_than_margin_when_short() {
        let opening_price = Decimal::from(30_000);
        let closing_price = Decimal::from(100_000);
        let quantity = 60_000.0;
        let long_leverage = 2.0;
        let short_leverage = 3.0;

        let margin = calculate_margin(opening_price, quantity, short_leverage);

        let pnl_short = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Short,
        )
        .unwrap();

        assert_eq!(pnl_short, (margin as i64).neg());
    }

    #[test]
    fn assert_to_not_lose_more_than_margin_when_long() {
        let opening_price = Decimal::from(30_000);
        let closing_price = Decimal::from(1);
        let quantity = 60_000.0;
        let long_leverage = 5.0;
        let short_leverage = 1.0;

        let margin = calculate_margin(opening_price, quantity, long_leverage);
        let pnl_short = calculate_pnl(
            opening_price,
            closing_price,
            quantity,
            long_leverage,
            short_leverage,
            Direction::Long,
        )
        .unwrap();

        assert_eq!(pnl_short, (margin as i64).neg());
    }
}
