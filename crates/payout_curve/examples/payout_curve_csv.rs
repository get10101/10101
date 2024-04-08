#![allow(clippy::unwrap_used)]

use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use payout_curve::build_inverse_payout_function;
use payout_curve::PartyParams;
use payout_curve::PayoutPoint;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::fs::File;
use std::ops::Mul;
use trade::cfd::calculate_long_bankruptcy_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_pnl;
use trade::cfd::calculate_short_bankruptcy_price;
use trade::Direction;

/// The example below will export the computed payout curve and how it should look like as CSV.
///
/// Use gnuplot to create a chart for it. An example gnuplot file has been provided
/// [`payout_curve.pg`]
fn main() -> Result<()> {
    let initial_price = dec!(30_000);
    let quantity = 30_000.0;
    let leverage_short = 2.0;
    let leverage_long = 2.0;

    let price_params = {
        let short_liquidation_price = calculate_short_bankruptcy_price(
            Decimal::from_f32(leverage_short).expect("to be able to parse f32"),
            initial_price,
        );

        let long_liquidation_price = calculate_long_bankruptcy_price(
            Decimal::from_f32(leverage_long).expect("to be able to parse f32"),
            initial_price,
        );

        payout_curve::PriceParams::new_btc_usd(
            initial_price,
            long_liquidation_price,
            short_liquidation_price,
        )?
    };

    // Fee is e.g. 0.3% * quantity / initial_price = 0.003 BTC = 300_000 sats.
    //
    // We compute it here so that can easily adjust the example.
    let fee_offer = {
        let fee = dec!(0.3) * Decimal::from_f32(quantity).expect("to be able to parse into dec")
            / initial_price;

        let fee = fee
            .mul(dec!(100_000_000))
            .to_u64()
            .expect("to fit into u64");

        Amount::from_sat(fee)
    };

    let margin_short = Amount::from_sat(calculate_margin(initial_price, quantity, leverage_short));
    let margin_long = Amount::from_sat(calculate_margin(initial_price, quantity, leverage_long));

    let direction_offer = Direction::Long;

    let (party_params_offer, party_params_accept) = match direction_offer {
        Direction::Long => (
            payout_curve::PartyParams::new(margin_long, fee_offer),
            payout_curve::PartyParams::new(margin_short, Amount::ZERO),
        ),
        Direction::Short => (
            payout_curve::PartyParams::new(margin_short, fee_offer),
            payout_curve::PartyParams::new(margin_long, Amount::ZERO),
        ),
    };

    let total_collateral =
        party_params_offer.total_collateral() + party_params_accept.total_collateral();

    let payout_points_offer_long = build_inverse_payout_function(
        quantity,
        party_params_offer,
        party_params_accept,
        price_params,
        direction_offer,
    )?;

    discretized_payouts_as_csv(
        "./crates/payout_curve/examples/discretized_long.csv",
        payout_points_offer_long.clone(),
        total_collateral,
    )?;

    let direction_offer = Direction::Short;

    let (party_params_offer, party_params_accept) = match direction_offer {
        Direction::Long => (
            payout_curve::PartyParams::new(margin_long, fee_offer),
            payout_curve::PartyParams::new(margin_short, Amount::ZERO),
        ),
        Direction::Short => (
            payout_curve::PartyParams::new(margin_short, fee_offer),
            payout_curve::PartyParams::new(margin_long, Amount::ZERO),
        ),
    };

    let total_collateral =
        party_params_offer.total_collateral() + party_params_accept.total_collateral();

    let payout_points_offer_short = build_inverse_payout_function(
        quantity,
        party_params_offer,
        party_params_accept,
        price_params,
        direction_offer,
    )?;

    discretized_payouts_as_csv(
        "./crates/payout_curve/examples/discretized_short.csv",
        payout_points_offer_short.clone(),
        total_collateral,
    )?;

    computed_payout_curve(
        party_params_offer,
        party_params_accept,
        "./crates/payout_curve/examples/computed_payout_long.csv",
        payout_points_offer_long,
    )?;

    computed_payout_curve(
        party_params_accept,
        party_params_offer,
        "./crates/payout_curve/examples/computed_payout_short.csv",
        payout_points_offer_short,
    )?;

    let leverage_long = Decimal::from_f32(leverage_long).context("to be able to parse f32")?;
    let leverage_short = Decimal::from_f32(leverage_short).context("to be able to parse f32")?;

    should_payouts_as_csv_short(
        margin_short.to_sat(),
        total_collateral,
        leverage_long,
        leverage_short,
        quantity,
        initial_price,
        "./crates/payout_curve/examples/should_short.csv",
        fee_offer.to_sat() as i64,
    )?;

    should_payouts_as_csv_long(
        margin_long.to_sat(),
        total_collateral,
        leverage_short,
        leverage_long,
        quantity,
        initial_price,
        "./crates/payout_curve/examples/should_long.csv",
        fee_offer.to_sat() as i64,
    )?;

    Ok(())
}

/// This is the discretized payout curve thrown into `to_rage_payouts` from rust-dlc, i.e. our DLCs
/// will be based on these points
#[allow(clippy::too_many_arguments)]
fn computed_payout_curve(
    party_params_coordinator: PartyParams,
    party_params_trader: PartyParams,
    csv_path: &str,
    payout_points: Vec<(PayoutPoint, PayoutPoint)>,
) -> Result<()> {
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
    let total_collateral =
        party_params_coordinator.total_collateral() + party_params_trader.total_collateral();
    let range_payouts = payout_function.to_range_payouts(
        total_collateral,
        &RoundingIntervals {
            intervals: vec![RoundingInterval {
                begin_interval: 0,
                rounding_mod: 1,
            }],
        },
    )?;

    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    wtr.write_record(["price", "payout_offer", "trader"])
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

/// This is our approach to discretize the payout, i.e. we only call our internal library
#[allow(clippy::too_many_arguments)]
fn discretized_payouts_as_csv(
    csv_path: &str,
    payout_points: Vec<(PayoutPoint, PayoutPoint)>,
    total_collateral: u64,
) -> Result<()> {
    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    wtr.write_record(["price", "payout_offer", "trader"])
        .context("to be able to write record")?;
    for (lower, _upper) in &payout_points {
        wtr.write_record([
            lower.event_outcome.to_string(),
            lower.outcome_payout.to_string(),
            (total_collateral - lower.outcome_payout).to_string(),
        ])?;
    }
    // need to add the last point because we ignored it explicitely above
    let last_point = payout_points[payout_points.len() - 1];
    wtr.write_record([
        last_point.1.event_outcome.to_string(),
        last_point.1.outcome_payout.to_string(),
        (total_collateral - last_point.1.outcome_payout).to_string(),
    ])?;
    wtr.flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn should_payouts_as_csv_short(
    coordinator_margin: u64,
    total_collateral: u64,
    leverage_long: Decimal,
    leverage_short: Decimal,
    quantity: f32,
    initial_price: Decimal,
    csv_path: &str,
    coordinator_collateral_reserve: i64,
) -> Result<()> {
    let coordinator_direction = Direction::Short;

    let total_collateral = total_collateral as i64;

    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);

    let long_liquidation_price = calculate_long_bankruptcy_price(leverage_long, initial_price);
    let short_liquidation_price = calculate_short_bankruptcy_price(leverage_short, initial_price);

    wtr.write_record(["price", "payout_offer", "trader"])?;
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

    let long_margin = calculate_margin(initial_price, quantity, leverage_long);
    let short_margin = calculate_margin(initial_price, quantity, leverage_short);

    for price in long_liquidation_price_i32..short_liquidation_price_i32 {
        let coordinator_payout = (((coordinator_margin as i64)
            + calculate_pnl(
                initial_price,
                Decimal::from(price),
                quantity,
                coordinator_direction,
                long_margin,
                short_margin,
            )?)
            + coordinator_collateral_reserve)
            .min(total_collateral);
        let trader_payout = total_collateral - coordinator_payout;
        wtr.write_record(&[
            price.to_string(),
            coordinator_payout.to_string(),
            trader_payout.to_string(),
        ])?;
    }
    {
        // upper liquidation range end
        let coordinator_payout = (((coordinator_margin as i64)
            + calculate_pnl(
                initial_price,
                short_liquidation_price,
                quantity,
                coordinator_direction,
                long_margin,
                short_margin,
            )?)
            + coordinator_collateral_reserve)
            .min(total_collateral);
        let trader_payout = total_collateral - coordinator_payout;
        wtr.write_record(&[
            short_liquidation_price.to_string(),
            coordinator_payout.to_string(),
            trader_payout.to_string(),
        ])?;
    }

    {
        // upper range end to get to 100k
        let coordinator_payout = (((coordinator_margin as i64)
            + calculate_pnl(
                initial_price,
                Decimal::from(100_000),
                quantity,
                coordinator_direction,
                long_margin,
                short_margin,
            )?)
            + coordinator_collateral_reserve)
            .min(total_collateral);
        let trader_payout = total_collateral - coordinator_payout;
        wtr.write_record(&[
            100_000.to_string(),
            coordinator_payout.to_string(),
            trader_payout.to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn should_payouts_as_csv_long(
    coordinator_margin: u64,
    total_collateral: u64,
    leverage_long: Decimal,
    leverage_short: Decimal,
    quantity: f32,
    initial_price: Decimal,
    csv_path: &str,
    coordinator_collateral_reserve: i64,
) -> Result<()> {
    let coordinator_direction = Direction::Long;

    let total_collateral = total_collateral as i64;

    let file = File::create(csv_path)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);

    let long_liquidation_price = calculate_long_bankruptcy_price(leverage_long, initial_price);
    let short_liquidation_price = calculate_short_bankruptcy_price(leverage_short, initial_price);

    wtr.write_record(["price", "payout_offer", "trader"])?;
    wtr.write_record(&[
        0.to_string(),
        coordinator_collateral_reserve.to_string(),
        (total_collateral - coordinator_collateral_reserve).to_string(),
    ])?;
    wtr.write_record(&[
        long_liquidation_price.to_string(),
        coordinator_collateral_reserve.to_string(),
        (total_collateral - coordinator_collateral_reserve).to_string(),
    ])?;

    let long_liquidation_price_i32 = long_liquidation_price
        .to_i32()
        .expect("to be able to convert");
    let short_liquidation_price_i32 = short_liquidation_price
        .to_i32()
        .expect("to be able to convert");
    let leverage_long = leverage_long.to_f32().expect("to be able to convert");
    let leverage_short = leverage_short.to_f32().expect("to be able to convert");

    let long_margin = calculate_margin(initial_price, quantity, leverage_long);
    let short_margin = calculate_margin(initial_price, quantity, leverage_short);

    for price in long_liquidation_price_i32..short_liquidation_price_i32 {
        let coordinator_payout = (((coordinator_margin as i64)
            + calculate_pnl(
                initial_price,
                Decimal::from(price),
                quantity,
                coordinator_direction,
                long_margin,
                short_margin,
            )?)
            + coordinator_collateral_reserve)
            .min(total_collateral);

        let trader_payout = total_collateral - coordinator_payout;
        wtr.write_record(&[
            price.to_string(),
            coordinator_payout.to_string(),
            trader_payout.min(total_collateral).max(0).to_string(),
        ])?;
    }
    {
        // upper range end to upper liquidation point
        let coordinator_payout = (((coordinator_margin as i64)
            + calculate_pnl(
                initial_price,
                short_liquidation_price,
                quantity,
                coordinator_direction,
                long_margin,
                short_margin,
            )?)
            + coordinator_collateral_reserve)
            .min(total_collateral);
        let trader_payout = (total_collateral - coordinator_payout).max(0);
        wtr.write_record(&[
            short_liquidation_price.to_string(),
            coordinator_payout.to_string(),
            trader_payout.max(0).min(total_collateral).to_string(),
        ])?;
    }
    {
        // upper range end to get to 100k
        let coordinator_payout = (((coordinator_margin as i64)
            + calculate_pnl(
                initial_price,
                Decimal::from(100_000),
                quantity,
                coordinator_direction,
                long_margin,
                short_margin,
            )?)
            + coordinator_collateral_reserve)
            .min(total_collateral);
        let trader_payout = total_collateral - coordinator_payout;
        wtr.write_record(&[
            100_000.to_string(),
            coordinator_payout.to_string(),
            (trader_payout).min(total_collateral).max(0).to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}
