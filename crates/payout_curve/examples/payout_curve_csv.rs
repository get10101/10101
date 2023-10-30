use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
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
use trade::cfd::calculate_pnl;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;

/// The example below will export the computed payout curve and how it should look like as CSV.
///
/// Use gnuplot to create a chart for it. An example gnuplot file has been provided
/// [`payout_curve.pg`]
fn main() -> Result<()> {
    let coordinator_collateral = Amount::ONE_BTC.to_sat();
    let trader_collateral = Amount::ONE_BTC.to_sat();
    let initial_price = dec!(30_000);
    let leverage_trader = 2.0;
    let leverage_coordinator = 2.0;
    let fee = 0;
    let quantity = 60_000.0;

    discretized_payouts_as_csv(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        Direction::Long,
        "./crates/payout_curve/examples/coordinator_long.csv",
    )?;
    discretized_payouts_as_csv(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        Direction::Short,
        "./crates/payout_curve/examples/coordinator_short.csv",
    )?;

    actual_payouts_as_csv(
        coordinator_collateral,
        trader_collateral,
        Direction::Short,
        Decimal::from_f32(leverage_trader).context("to be able to parse f32")?,
        Decimal::from_f32(leverage_coordinator).context("to be able to parse f32")?,
        quantity,
        initial_price,
        "./crates/payout_curve/examples/should.csv",
    )?;

    computed_payout_curve(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        Direction::Long,
        "./crates/payout_curve/examples/computed_payout.csv",
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
    wtr.write_record([
        payout_points[payout_points.len() - 1]
            .1
            .event_outcome
            .to_string(),
        payout_points[payout_points.len() - 1]
            .1
            .outcome_payout
            .to_string(),
    ])?;
    wtr.flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn actual_payouts_as_csv(
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
    for i in long_liquidation_price
        .to_i32()
        .expect("to be able to convert")
        ..=short_liquidation_price
            .to_i32()
            .expect("to be able to convert")
    {
        wtr.write_record(&[
            i.to_string(),
            ((coordinator_collateral as i64)
                + calculate_pnl(
                    initial_price,
                    Decimal::from(i),
                    quantity,
                    leverage_long.to_f32().expect("to be able to convert"),
                    leverage_short.to_f32().expect("to be able to convert"),
                    coordinator_direction,
                )?)
            .to_string(),
            ((trader_collateral as i64)
                + calculate_pnl(
                    initial_price,
                    Decimal::from(i),
                    quantity,
                    leverage_long.to_f32().expect("to be able to convert"),
                    leverage_short.to_f32().expect("to be able to convert"),
                    coordinator_direction.opposite(),
                )?)
            .to_string(),
        ])?;
    }
    wtr.write_record(&[
        short_liquidation_price.to_string(),
        0.to_string(),
        total_collateral.to_string(),
    ])?;
    wtr.write_record(&[
        100_000.to_string(),
        0.to_string(),
        total_collateral.to_string(),
    ])?;
    wtr.flush()?;
    Ok(())
}
