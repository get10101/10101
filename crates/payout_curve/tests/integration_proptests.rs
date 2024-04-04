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
use payout_curve::PriceParams;
use proptest::prelude::*;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::fs::File;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use trade::cfd::calculate_long_bankruptcy_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_bankruptcy_price;
use trade::Direction;

/// set this to true to export test data to csv files
const PRINT_CSV: bool = false;

/// Taken from a past crash.
#[test]
fn calculating_payout_curve_doesnt_crash_1() {
    let coordinator_direction = Direction::Short;

    let initial_price = Decimal::from_u64(26986).unwrap();
    let leverage_trader = 3.0;
    let leverage_coordinator = 3.0;
    let collateral_reserve_offer = 0;
    let quantity = 1.0;

    let coordinator_margin = calculate_margin(initial_price, quantity, leverage_coordinator);
    let trader_margin = calculate_margin(initial_price, quantity, leverage_trader);

    let (leverage_long, leverage_short) = match coordinator_direction {
        Direction::Long => (leverage_coordinator, leverage_trader),
        Direction::Short => (leverage_trader, leverage_coordinator),
    };

    let long_liquidation_price = calculate_long_bankruptcy_price(
        Decimal::from_f32(leverage_long).expect("to be able to parse f32"),
        initial_price,
    );
    let short_liquidation_price = calculate_short_bankruptcy_price(
        Decimal::from_f32(leverage_short).expect("to be able to parse f32"),
        initial_price,
    );

    // act: we only test that this does not panic
    computed_payout_curve(
        quantity,
        coordinator_margin,
        trader_margin,
        initial_price,
        collateral_reserve_offer,
        coordinator_direction,
        long_liquidation_price,
        short_liquidation_price,
    )
    .unwrap();
}

/// Taken from a past crash.
#[test]
fn calculating_payout_curve_doesnt_crash_2() {
    let coordinator_direction = Direction::Short;

    let initial_price = dec!(30_000.0);
    let leverage_trader = 1.0;
    let leverage_coordinator = 1.0;
    let collateral_reserve_offer = 0;
    let quantity = 10.0;

    let coordinator_collateral = calculate_margin(initial_price, quantity, leverage_coordinator);
    let trader_collateral = calculate_margin(initial_price, quantity, leverage_trader);

    let (leverage_long, leverage_short) = match coordinator_direction {
        Direction::Long => (leverage_coordinator, leverage_trader),
        Direction::Short => (leverage_trader, leverage_coordinator),
    };

    let long_liquidation_price = calculate_long_bankruptcy_price(
        Decimal::from_f32(leverage_long).expect("to be able to parse f32"),
        initial_price,
    );
    let short_liquidation_price = calculate_short_bankruptcy_price(
        Decimal::from_f32(leverage_short).expect("to be able to parse f32"),
        initial_price,
    );

    // act: we only test that this does not panic
    computed_payout_curve(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        collateral_reserve_offer,
        coordinator_direction,
        long_liquidation_price,
        short_liquidation_price,
    )
    .unwrap();
}

/// Taken from a past crash.
#[test]
fn calculating_payout_curve_doesnt_crash_3() {
    let coordinator_direction = Direction::Short;

    let initial_price = dec!(34586);
    let leverage_trader = 2.0;
    let leverage_coordinator = 2.0;
    let collateral_reserve_offer = 0;
    let quantity = 1.0;

    let coordinator_collateral = calculate_margin(initial_price, quantity, leverage_coordinator);
    let trader_collateral = calculate_margin(initial_price, quantity, leverage_trader);

    let (leverage_long, leverage_short) = match coordinator_direction {
        Direction::Long => (leverage_coordinator, leverage_trader),
        Direction::Short => (leverage_trader, leverage_coordinator),
    };

    let long_liquidation_price = calculate_long_bankruptcy_price(
        Decimal::from_f32(leverage_long).expect("to be able to parse f32"),
        initial_price,
    );
    let short_liquidation_price = calculate_short_bankruptcy_price(
        Decimal::from_f32(leverage_short).expect("to be able to parse f32"),
        initial_price,
    );

    // act: we only test that this does not panic
    computed_payout_curve(
        quantity,
        coordinator_collateral,
        trader_collateral,
        initial_price,
        collateral_reserve_offer,
        coordinator_direction,
        long_liquidation_price,
        short_liquidation_price,
    )
    .unwrap();
}

proptest! {
    #[test]
    fn calculating_lower_bound_doesnt_crash(
         leverage_trader in 1u8..5,
         direction in 0..2,
    ) {
        init_tracing_for_test();
        let leverage_trader = leverage_trader as f32;
        let coordinator_direction = if direction == 0 {
            Direction::Short
        }
        else {
            Direction::Long
        };

        let initial_price = dec!(30_000.0);
        let leverage_coordinator = 2.0;
        let quantity = 10.0;
        let fee = 0;

        let coordinator_margin = calculate_margin(initial_price, quantity, leverage_coordinator);
        let trader_margin = calculate_margin(initial_price, quantity, leverage_trader);

        let (leverage_long, leverage_short) = match coordinator_direction {
            Direction::Long => (leverage_coordinator, leverage_trader),
            Direction::Short => (leverage_trader, leverage_coordinator),
        };

        let long_liquidation_price = calculate_long_bankruptcy_price(
            Decimal::from_f32(leverage_long).expect("to be able to parse f32"),
            initial_price,
        );
        let short_liquidation_price = calculate_short_bankruptcy_price(
            Decimal::from_f32(leverage_short).expect("to be able to parse f32"),
            initial_price,
        );

        tracing::info!(
            leverage_trader,
            ?coordinator_direction,
            initial_price = initial_price.to_string(),
            leverage_coordinator,
            quantity,
            fee,
            coordinator_margin,
            trader_margin,
            ?long_liquidation_price,
            ?short_liquidation_price,
            "Started computing payout curve"
        );

        // act: we only test that this does not panic
        let now = std::time::Instant::now();

        computed_payout_curve(
            quantity,
            coordinator_margin,
            trader_margin,
            initial_price,
            fee,
            coordinator_direction,
            long_liquidation_price,
            short_liquidation_price,
        ).unwrap();

        tracing::info!(
            elapsed_ms = %now.elapsed().as_millis(),
            "Computed payout curve"
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn computed_payout_curve(
    quantity: f32,
    coordinator_margin: u64,
    trader_margin: u64,
    initial_price: Decimal,
    coordinator_collateral_reserve: u64,
    coordinator_direction: Direction,
    long_liquidation_price: Decimal,
    short_liquidation_price: Decimal,
) -> Result<()> {
    let price_params = PriceParams::new_btc_usd(
        initial_price,
        long_liquidation_price,
        short_liquidation_price,
    )?;

    let party_params_coordinator = PartyParams::new(
        Amount::from_sat(coordinator_margin),
        Amount::from_sat(coordinator_collateral_reserve),
    );
    let party_params_trader = PartyParams::new(Amount::from_sat(trader_margin), Amount::ZERO);

    let payout_points = build_inverse_payout_function(
        quantity,
        party_params_coordinator,
        party_params_trader,
        price_params,
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

    let total_collateral =
        party_params_coordinator.total_collateral() + party_params_trader.total_collateral();
    let _ = payout_function.to_range_payouts(
        total_collateral,
        &RoundingIntervals {
            intervals: vec![RoundingInterval {
                begin_interval: 0,
                rounding_mod: 1,
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
