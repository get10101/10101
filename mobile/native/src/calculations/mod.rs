use crate::common::api::Direction;
use dlc::channel::sub_channel::LN_GLUE_TX_WEIGHT;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use trade::cfd;

/// Calculate the collateral in BTC.
pub fn calculate_margin(opening_price: f64, quantity: f64, leverage: f64) -> u64 {
    cfd::calculate_margin(opening_price, quantity, leverage)
}

/// Calculate the quantity from price, collateral and leverage
/// Margin in sats, calculation in BTC
pub fn calculate_quantity(opening_price: f64, margin: u64, leverage: f64) -> f64 {
    cfd::calculate_quantity(opening_price, margin, leverage)
}

pub fn calculate_liquidation_price(price: f64, leverage: f64, direction: Direction) -> f64 {
    let initial_price = Decimal::try_from(price).expect("Price to fit");

    tracing::trace!("Initial price: {}", price);

    let leverage = Decimal::try_from(leverage).expect("leverage to fix into decimal");

    let liquidation_price = match direction {
        Direction::Long => cfd::calculate_long_liquidation_price(leverage, initial_price),
        Direction::Short => cfd::calculate_short_liquidation_price(leverage, initial_price),
    };

    let liquidation_price = liquidation_price.to_f64().expect("price to fit into f64");
    tracing::trace!("Liquidation_price: {liquidation_price}");

    liquidation_price
}

pub fn calculate_fees() -> u64 {
    // TODO: this should probably come from a configuration.
    let fee_rate = 2;
    let total_fee = (dlc::channel::sub_channel::dlc_channel_and_split_fee(fee_rate)
        + dlc::util::weight_to_fee(LN_GLUE_TX_WEIGHT, fee_rate)) as f64;

    // the party that offers the trade will pay the extra sat in case the fees are uneven.
    (total_fee / 2.0).ceil() as u64
}
