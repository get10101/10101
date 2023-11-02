use anyhow::Context;
use anyhow::Result;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use payout_curve::build_inverse_payout_function;
use payout_curve::ROUNDING_PERCENT;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::fs::File;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_pnl;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;

/// The example below will export the computed payout curve and how it should look like as CSV.
///
/// Use gnuplot to create a chart for it. An example gnuplot file has been provided
/// [`payout_curve.pg`]
fn main() -> Result<()> {
    let initial_price = dec!(30_000);
    let quantity = 30_000.0;
    let leverage_short = 3.0;
    let leverage_long = 2.0;
    let fee = 0;
    let margin_short = calculate_margin(initial_price, quantity, leverage_short);
    let margin_long = calculate_margin(initial_price, quantity, leverage_long);

    // offerer is long
    discretized_payouts_as_csv(
        quantity,
        margin_long,
        margin_short,
        initial_price,
        leverage_short,
        leverage_long,
        fee,
        Direction::Long,
        "./crates/payout_curve/examples/offerer_long.csv",
    )?;

    // offerer is short
    discretized_payouts_as_csv(
        quantity,
        margin_short,
        margin_long,
        initial_price,
        leverage_long,
        leverage_short,
        fee,
        Direction::Short,
        "./crates/payout_curve/examples/offerer_short.csv",
    )?;

    computed_payout_curve(
        quantity,
        margin_long,
        margin_short,
        initial_price,
        leverage_short,
        leverage_long,
        fee,
        Direction::Long,
        "./crates/payout_curve/examples/computed_payout.csv",
    )?;

    let leverage_long = Decimal::from_f32(leverage_long).context("to be able to parse f32")?;
    let leverage_short = Decimal::from_f32(leverage_short).context("to be able to parse f32")?;

    should_payouts_as_csv(
        margin_short,
        margin_long,
        Direction::Short,
        leverage_long,
        leverage_short,
        quantity,
        initial_price,
        "./crates/payout_curve/examples/should.csv",
    )?;

    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn computed_payout_curve(
    quantity: f32,
    coordinator_collateral: u64,
    trader_collateral: u64,
    initial_price: Decimal,
    leverage_trader: f32,
    leverage_coordinator: f32,
    fee: u64,
    coordinator_direction: Direction,
    csv_path: &str,
) -> Result<()> {
    let payout_points = build_inverse_payout_function(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        coordinator_direction,
    )?;

    let mut pieces = vec![];
    for (lower, upper) in payout_points {
        let lower_range = PolynomialPayoutCurvePiece::new(vec![
            dlc_manager::payout_curve::PayoutPoint {
                event_outcome: lower.event_outcome,
                outcome_payout: lower.outcome_payout,
                extra_precision: lower.extra_precision,
            },
            dlc_manager::payout_curve::PayoutPoint {
                event_outcome: upper.event_outcome,
                outcome_payout: upper.outcome_payout,
                extra_precision: upper.extra_precision,
            },
        ])?;
        pieces.push(PayoutFunctionPiece::PolynomialPayoutCurvePiece(lower_range));
    }

    let payout_function =
        PayoutFunction::new(pieces).context("could not create payout function")?;
    let total_collateral = coordinator_collateral + trader_collateral;
    let range_payouts = payout_function.to_range_payouts(
        total_collateral,
        &RoundingIntervals {
            intervals: vec![RoundingInterval {
                begin_interval: 0,
                rounding_mod: (total_collateral as f32 * ROUNDING_PERCENT) as u64,
            }],
        },
    )?;

    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    wtr.write_record(["price", "payout_offer", "payout_accept"])
        .context("to be able to write record")?;
    for payout in &range_payouts {
        wtr.write_record([
            payout.start.to_string(),
            payout.payout.offer.to_string(),
            payout.payout.accept.to_string(),
        ])?;
    }
    wtr.flush()?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn discretized_payouts_as_csv(
    quantity: f32,
    offer_collateral: u64,
    accept_collateral: u64,
    initial_price: Decimal,
    leverage_accept: f32,
    leverage_offer: f32,
    fee: u64,
    offer_direction: Direction,
    csv_path: &str,
) -> Result<()> {
    let payout_points = build_inverse_payout_function(
        quantity,
        offer_collateral,
        accept_collateral,
        initial_price,
        leverage_accept,
        leverage_offer,
        fee,
        offer_direction,
    )?;

    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    wtr.write_record(["price", "payout_offer"])
        .context("to be able to write record")?;
    for (lower, _upper) in &payout_points {
        wtr.write_record([
            lower.event_outcome.to_string(),
            lower.outcome_payout.to_string(),
        ])?;
    }
    // need to add the last point because we ignored it explicitely above
    let last_point = payout_points[payout_points.len() - 1].clone();
    wtr.write_record([
        last_point.1.event_outcome.to_string(),
        last_point.1.outcome_payout.to_string(),
    ])?;
    wtr.flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn should_payouts_as_csv(
    coordinator_collateral: u64,
    trader_collateral: u64,
    coordinator_direction: Direction,
    leverage_long: Decimal,
    leverage_short: Decimal,
    quantity: f32,
    initial_price: Decimal,
    csv_path: &str,
) -> Result<()> {
    let total_collateral = coordinator_collateral + trader_collateral;

    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);

    let long_liquidation_price = calculate_long_liquidation_price(leverage_long, initial_price);
    let short_liquidation_price = calculate_short_liquidation_price(leverage_short, initial_price);

    wtr.write_record(["start", "coordinator", "trader"])?;
    wtr.write_record(&[0.to_string(), total_collateral.to_string(), 0.to_string()])?;
    wtr.write_record(&[
        long_liquidation_price.to_string(),
        total_collateral.to_string(),
        0.to_string(),
    ])?;

    let long_liquidation_price_i32 = long_liquidation_price
        .to_i32()
        .expect("to be able to convert");
    let short_liquidation_price_i32 = short_liquidation_price
        .to_i32()
        .expect("to be able to convert");
    let leverage_long = leverage_long.to_f32().expect("to be able to convert");
    let leverage_short = leverage_short.to_f32().expect("to be able to convert");

    for price in long_liquidation_price_i32..short_liquidation_price_i32 {
        wtr.write_record(&[
            price.to_string(),
            ((coordinator_collateral as i64)
                + calculate_pnl(
                    initial_price,
                    Decimal::from(price),
                    quantity,
                    leverage_long,
                    leverage_short,
                    coordinator_direction,
                )?)
            .to_string(),
            ((trader_collateral as i64)
                + calculate_pnl(
                    initial_price,
                    Decimal::from(price),
                    quantity,
                    leverage_long,
                    leverage_short,
                    coordinator_direction.opposite(),
                )?)
            .to_string(),
        ])?;
    }
    wtr.write_record(&[
        short_liquidation_price.to_string(),
        ((coordinator_collateral as i64)
            + calculate_pnl(
                initial_price,
                short_liquidation_price,
                quantity,
                leverage_long,
                leverage_short,
                coordinator_direction,
            )?)
        .to_string(),
        ((trader_collateral as i64)
            + calculate_pnl(
                initial_price,
                short_liquidation_price,
                quantity,
                leverage_long,
                leverage_short,
                coordinator_direction.opposite(),
            )?)
        .to_string(),
    ])?;
    wtr.write_record(&[
        100_000.to_string(),
        ((coordinator_collateral as i64)
            + calculate_pnl(
                initial_price,
                Decimal::from(100_000),
                quantity,
                leverage_long,
                leverage_short,
                coordinator_direction,
            )?)
        .to_string(),
        ((trader_collateral as i64)
            + calculate_pnl(
                initial_price,
                Decimal::from(100_000),
                quantity,
                leverage_long,
                leverage_short,
                coordinator_direction.opposite(),
            )?)
        .to_string(),
    ])?;
    wtr.flush()?;
    Ok(())
}
