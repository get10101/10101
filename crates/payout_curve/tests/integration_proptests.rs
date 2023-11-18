use anyhow::Context;
use anyhow::Result;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use payout_curve::build_inverse_payout_function;
use payout_curve::ROUNDING_PERCENT;
use proptest::prelude::*;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::fs::File;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use trade::cfd::calculate_margin;
use trade::Direction;

/// set this to true to export test data to csv files
const PRINT_CSV: bool = false;

/// taken from a past crash
#[test]
fn calculating_payout_curve_doesnt_crash_1() {
    let initial_price = Decimal::from_u64(26986).unwrap();
    let leverage_trader = 3.0;
    let leverage_coordinator = 3.0;
    let fee = 0;
    let coordinator_direction = Direction::Short;
    let quantity = 1.0;

    let coordinator_collateral = calculate_margin(initial_price, quantity, leverage_coordinator);
    let trader_collateral = calculate_margin(initial_price, quantity, leverage_trader);

    // act: we only test that this does not panic
    computed_payout_curve(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        coordinator_direction,
    )
    .unwrap();
}

/// taken from a past crash
#[test]
fn calculating_payout_curve_doesnt_crash_2() {
    let initial_price = dec!(30_000.0);
    let leverage_trader = 1.0;
    let leverage_coordinator = 1.0;
    let fee = 0;
    let coordinator_direction = Direction::Short;
    let quantity = 10.0;

    let coordinator_collateral = calculate_margin(initial_price, quantity, leverage_coordinator);
    let trader_collateral = calculate_margin(initial_price, quantity, leverage_trader);

    // act: we only test that this does not panic
    computed_payout_curve(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        coordinator_direction,
    )
    .unwrap();
}
/// taken from a past crash
#[test]
fn calculating_payout_curve_doesnt_crash_3() {
    let initial_price = dec!(34586);
    let leverage_trader = 2.0;
    let leverage_coordinator = 2.0;
    let fee = 0;
    let coordinator_direction = Direction::Short;
    let quantity = 1.0;

    let coordinator_collateral = calculate_margin(initial_price, quantity, leverage_coordinator);
    let trader_collateral = calculate_margin(initial_price, quantity, leverage_trader);

    // act: we only test that this does not panic
    computed_payout_curve(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        fee,
        coordinator_direction,
    )
    .unwrap();
}

proptest! {
    #[test]
    fn calculating_lower_bound_doesnt_crash(
         trader_leverage in 1u8..5,
         direction in 0..2,
    ) {
        init_tracing_for_test();
        let leverage_trader= trader_leverage as f32;
        let coordinator_direction = if direction == 0 {
            Direction::Short
        }
        else {
            Direction::Long
        };

        let initial_price = dec!(30_000.0);
        let leverage_coordinator= 2.0;
        let quantity = 10.0;
        let fee= 0;

        let coordinator_collateral= calculate_margin(initial_price, quantity, leverage_coordinator);
        let trader_collateral= calculate_margin(initial_price, quantity, leverage_trader);

        let now = std::time::Instant::now();
        let direction_string = format!("{:?}", coordinator_direction);
        tracing::info!(
            leverage_trader,
            coordinator_direction = direction_string,
            initial_price = initial_price.to_string(),
            leverage_coordinator,
            quantity,
            fee,
            coordinator_collateral,
            trader_collateral,
            "Started computing payout curve");


        // act: we only test that this does not panic
        computed_payout_curve(
            quantity,
            coordinator_collateral,
            trader_collateral,
            initial_price,
            leverage_trader,
            leverage_coordinator,
            fee,
            coordinator_direction,
        ).unwrap();
        let elapsed_ms = now.elapsed().as_millis();
        tracing::info!(
            elapsed_ms,
            "Took total");
    }
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

    let start = SystemTime::now();
    let now = start.duration_since(UNIX_EPOCH)?;

    let mut pieces = vec![];
    for (lower, upper) in &payout_points {
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

    if PRINT_CSV {
        let file = File::create(format!("./testrun-{}.csv", now.as_millis()))?;
        let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
        wtr.write_record(["lower", "upper", "lower payout", "upper payout"])
            .context("to be able to write record")?;

        for (lower, upper) in payout_points {
            wtr.write_record([
                lower.event_outcome.to_string(),
                upper.event_outcome.to_string(),
                lower.outcome_payout.to_string(),
                upper.outcome_payout.to_string(),
            ])?;
        }
        wtr.flush()?;
    }

    let payout_function =
        PayoutFunction::new(pieces).context("could not create payout function")?;

    let total_collateral = coordinator_collateral + trader_collateral;
    let _ = payout_function.to_range_payouts(
        total_collateral,
        &RoundingIntervals {
            intervals: vec![RoundingInterval {
                begin_interval: 0,
                rounding_mod: (total_collateral as f32 * ROUNDING_PERCENT) as u64,
            }],
        },
    )?;

    Ok(())
}

/// Initialise tracing for tests
#[cfg(test)]
pub(crate) fn init_tracing_for_test() {
    static TRACING_TEST_SUBSCRIBER: std::sync::Once = std::sync::Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .init()
    })
}
