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

/// PnL is calculated using the margin without fees to show the effective profit or loss.
pub fn calculate_pnl(
    opening_price: f32,
    closing_price: Price,
    quantity: f32,
    leverage: f32,
    direction: Direction,
) -> Result<i64> {
    // FIXME: We can no longer assume that the coordinator always has the same leverage! It needs to
    // be passed in as an argument. Unfortunately the coordinator leverage is not passed around at
    // the moment. Perhaps we should add it to the `TradeParams`.
    let (long_leverage, short_leverage) = match direction {
        Direction::Long => (leverage, 2.0),
        Direction::Short => (2.0, leverage),
    };

    let long_margin = calculate_margin(opening_price, quantity, long_leverage);
    let short_margin = calculate_margin(opening_price, quantity, short_leverage);

    let opening_price = Decimal::try_from(opening_price).expect("price to fit into decimal");
    let closing_price = closing_price.get_price_for_direction(direction.opposite());

    cfd::calculate_pnl(
        opening_price,
        closing_price,
        quantity,
        direction,
        long_margin,
        short_margin,
    )
}

pub fn calculate_liquidation_price(
    price: f32,
    leverage: f32,
    direction: Direction,
    maintenance_margin_rate: Decimal,
) -> f32 {
    let initial_price = Decimal::try_from(price).expect("Price to fit");

    tracing::trace!("Initial price: {}", price);

    let leverage = Decimal::try_from(leverage).expect("leverage to fix into decimal");

    let liquidation_price = match direction {
        Direction::Long => {
            cfd::calculate_long_liquidation_price(leverage, initial_price, maintenance_margin_rate)
        }
        Direction::Short => {
            cfd::calculate_short_liquidation_price(leverage, initial_price, maintenance_margin_rate)
        }
    };

    let liquidation_price = liquidation_price.to_f32().expect("price to fit into f32");
    tracing::trace!("Liquidation_price: {liquidation_price}");

    liquidation_price
}
