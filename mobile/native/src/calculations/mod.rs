use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use trade::cfd;
use trade::Direction;
use trade::Price;

/// Calculate the collateral in BTC.
pub fn calculate_margin(opening_price: f32, quantity: f32, leverage: f32) -> u64 {
    let opening_price = Decimal::try_from(opening_price).expect("price to fit into decimal");
    cfd::calculate_margin(opening_price, quantity, leverage)
}

/// Calculate the quantity from price, collateral and leverage
/// Margin in sats, calculation in BTC
pub fn calculate_quantity(opening_price: f32, margin: u64, leverage: f32) -> f32 {
    cfd::calculate_quantity(opening_price, margin, leverage)
}

pub fn calculate_pnl(
    opening_price: f32,
    closing_price: Price,
    quantity: f32,
    leverage: f32,
    direction: Direction,
) -> Result<i64> {
    let (long_leverage, short_leverage) = match direction {
        Direction::Long => (leverage, 1.0),
        Direction::Short => (1.0, leverage),
    };

    let opening_price = Decimal::try_from(opening_price).expect("price to fit into decimal");
    let closing_price = closing_price.get_price_for_direction(direction.opposite());

    cfd::calculate_pnl(
        opening_price,
        closing_price,
        quantity,
        long_leverage,
        short_leverage,
        direction,
    )
}

pub fn calculate_liquidation_price(price: f32, leverage: f32, direction: Direction) -> f32 {
    let initial_price = Decimal::try_from(price).expect("Price to fit");

    tracing::trace!("Initial price: {}", price);

    let leverage = Decimal::try_from(leverage).expect("leverage to fix into decimal");

    let liquidation_price = match direction {
        Direction::Long => cfd::calculate_long_liquidation_price(leverage, initial_price),
        Direction::Short => cfd::calculate_short_liquidation_price(leverage, initial_price),
    };

    let liquidation_price = liquidation_price.to_f32().expect("price to fit into f32");
    tracing::trace!("Liquidation_price: {liquidation_price}");

    liquidation_price
}
