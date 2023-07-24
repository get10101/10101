use crate::Direction;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin;
use bdk::bitcoin::Denomination;
use bdk::bitcoin::SignedAmount;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
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

// TODO: This was copied from ItchySats and adapted; we need tests for this!
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

        let uncapped_pnl = (quantity / opening_price) - (quantity / closing_price);
        let uncapped_pnl = uncapped_pnl
            .round_dp_with_strategy(8, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        let uncapped_pnl = uncapped_pnl
            .to_f64()
            .context("Could not convert Decimal to f64")?;

        SignedAmount::from_btc(uncapped_pnl)?.to_sat()
    };

    // TODO: Fees are still missing; see ItchySats FeeAccount

    let pnl = match direction {
        Direction::Long => uncapped_pnl_long.min(short_margin as i64),
        Direction::Short => uncapped_pnl_long.neg().min(long_margin as i64),
    };

    Ok(pnl)
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
        assert_eq!(pnl_long, -500000);
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
}
