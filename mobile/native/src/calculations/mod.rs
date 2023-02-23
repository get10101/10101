use crate::common::api::Direction;
use bdk::bitcoin;
use bdk::bitcoin::Denomination;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Calculate the margin in BTC.
pub fn calculate_margin(opening_price: f64, quantity: f64, leverage: f64) -> u64 {
    let quantity = Decimal::try_from(quantity).expect("quantity to fit into decimal");
    let open_price = Decimal::try_from(opening_price).expect("price to fit into decimal");
    let leverage = Decimal::try_from(leverage).expect("leverage to fix into decimal");

    if open_price == Decimal::ZERO || leverage == Decimal::ZERO {
        // just to avoid div by 0 errors
        return 0;
    }

    let margin = quantity / (open_price * leverage);

    // TODO: Shift the decimal without going into float
    let margin =
        margin.round_dp_with_strategy(8, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    let margin = margin.to_f64().expect("margin to fit into f64");

    bitcoin::Amount::from_btc(margin)
        .expect("margin to fit in amount")
        .to_sat()
}

/// Calculate the quantity from price, margin and leverage
/// Margin in sats, calculation in BTC
pub fn calculate_quantity(opening_price: f64, margin: u64, leverage: f64) -> f64 {
    let margin_amount = bitcoin::Amount::from_sat(margin);

    let margin = Decimal::try_from(margin_amount.to_float_in(Denomination::Bitcoin))
        .expect("margin to fit into decimal");
    let open_price = Decimal::try_from(opening_price).expect("price to fit into decimal");
    let leverage = Decimal::try_from(leverage).expect("leverage to fit into decimal");

    let quantity = margin * open_price * leverage;
    quantity.to_f64().expect("quantity to fit into f64")
}

pub fn calculate_liquidation_price(price: f64, leverage: f64, direction: Direction) -> f64 {
    let initial_price = Decimal::try_from(price).expect("Price to fit");

    tracing::trace!("Initial price: {}", price);

    let leverage = Decimal::try_from(leverage).expect("leverage to fix into decimal");

    let liquidation_price = match direction {
        Direction::Long => calculate_long_liquidation_price(leverage, initial_price),
        Direction::Short => calculate_short_liquidation_price(leverage, initial_price),
    };

    let liquidation_price = liquidation_price.to_f64().expect("price to fit into f64");
    tracing::trace!("Liquidation_price: {liquidation_price}");

    liquidation_price
}

fn calculate_long_liquidation_price(leverage: Decimal, price: Decimal) -> Decimal {
    price * leverage / (leverage + Decimal::ONE)
}

/// Calculate liquidation price for the party going short.
fn calculate_short_liquidation_price(leverage: Decimal, price: Decimal) -> Decimal {
    // If the leverage is equal to 1, the liquidation price will go towards infinity
    if leverage == Decimal::ONE {
        return rust_decimal_macros::dec!(21_000_000);
    }
    price * leverage / (leverage - Decimal::ONE)
}
