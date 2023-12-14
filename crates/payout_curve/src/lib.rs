use anyhow::Context;
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use trade::cfd::calculate_pnl;
use trade::cfd::BTCUSD_MAX_PRICE;
use trade::Direction;

/// We use this variable to indicate the step interval for our payout function. It should be
/// relative to the overall collateral so that we use the same amount of payouts for all intervals.
/// This means, the higher the overall collateral, the bigger the steps.
///
/// 0.01 means 1%, i.e. we always have ~100 payouts.
pub const ROUNDING_PERCENT: f32 = 0.01;

/// Defines the steps to take in the payout curve for one point. A step of 2 means, that two points
/// are $1 away from each other.
const PAYOUT_CURVE_DISCRETIZATION_STEPS: u64 = 20;

/// A payout point representing a payout for a given outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PayoutPoint {
    /// The event outcome.
    pub event_outcome: u64,
    /// The payout for the outcome.
    pub outcome_payout: u64,
    /// Extra precision to use when computing the payout.
    pub extra_precision: u16,
}

/// Build a [`PayoutFunction`] for an inverse perpetual future e.g. BTCUSD. Perspective is always
/// from the person who offers, i.e. in our case from the offerer.
///
/// Returns a Vec<(PayoutPoint, PayoutPoint)>>. Each tuple is meant to be put into one
/// [`dlc_manager::payout_curve::PolynomialPayoutCurvePiece`]
/// Note: `fee` is always paid towards the offerer
#[allow(clippy::too_many_arguments)]
pub fn build_inverse_payout_function(
    quantity: f32,
    offer_collateral: u64,
    accept_collateral: u64,
    initial_price: Decimal,
    offer_liquidation_price: Decimal,
    accept_liquidation_price: Decimal,
    fee: u64,
    offer_direction: Direction,
) -> Result<Vec<(PayoutPoint, PayoutPoint)>> {
    let mut pieces = vec![];
    let total_collateral = offer_collateral + accept_collateral;

    let (long_liquidation_price, short_liquidation_price) = match offer_direction {
        Direction::Long => (offer_liquidation_price, accept_liquidation_price),
        Direction::Short => (accept_liquidation_price, offer_liquidation_price),
    };
    let short_liquidation_price = short_liquidation_price.min(Decimal::from(BTCUSD_MAX_PRICE));

    let (long_liquidation_range_lower, long_liquidation_range_upper) =
        calculate_short_liquidation_interval_payouts(
            offer_direction,
            total_collateral,
            long_liquidation_price,
            fee,
        )?;

    let last_payout = long_liquidation_range_upper.clone();

    pieces.push((long_liquidation_range_lower, long_liquidation_range_upper));

    let mid_range = calculate_mid_range_payouts(
        accept_collateral,
        offer_collateral,
        initial_price,
        long_liquidation_price
            .to_u64()
            .expect("to fit dec into u64"),
        short_liquidation_price
            .to_u64()
            .expect("to fit dec into u64"),
        &last_payout,
        offer_direction,
        quantity,
        fee,
    )?;

    let (_, last_mid_range) = mid_range
        .last()
        .context("didn't have at least a single element in the mid range")?
        .clone();

    for (lower, upper) in mid_range {
        pieces.push((lower, upper));
    }
    // if the upper bound is already [`BTCUSD_MAX_PRICE`] we don't have to add the upper bound
    // anymore
    if last_mid_range.event_outcome < BTCUSD_MAX_PRICE {
        let upper_range_payout_points =
            calculate_upper_range_payouts(offer_direction, total_collateral, last_mid_range, fee)?;
        pieces.push(upper_range_payout_points);
    }

    Ok(pieces)
}

/// Calculates the mid range payout points between lower liquidation and upper liquidation
///
/// Returns tuples of payout points, first item is lower point, next item is higher point of two
/// points on the payout curve
#[allow(clippy::too_many_arguments)]
fn calculate_mid_range_payouts(
    accept_collateral: u64,
    offer_collateral: u64,
    initial_price: Decimal,
    lower_limit: u64,
    upper_limit: u64,
    last_payout: &PayoutPoint,
    offer_direction: Direction,
    quantity: f32,
    fee: u64,
) -> Result<Vec<(PayoutPoint, PayoutPoint)>> {
    let total_collateral = accept_collateral + offer_collateral;

    let (long_margin, short_margin) = match offer_direction {
        Direction::Long => (offer_collateral, accept_collateral),
        Direction::Short => (accept_collateral, offer_collateral),
    };

    let pieces = (lower_limit..upper_limit)
        .step_by(PAYOUT_CURVE_DISCRETIZATION_STEPS as usize)
        .map(|current_price| {
            let lower_event_outcome = current_price;
            let lower_event_outcome_payout = if current_price == lower_limit {
                // the last_payout includes already the fee. Hence, we need to subtract it here as
                // we add it again later
                (last_payout.outcome_payout - fee) as i64
            } else {
                offer_collateral as i64
                    + calculate_pnl(
                        initial_price,
                        Decimal::from(current_price),
                        quantity,
                        offer_direction,
                        long_margin,
                        short_margin,
                    )?
            };
            let lower_event_outcome_payout = if lower_event_outcome_payout <= 0 {
                // we can't get negative payouts
                0
            } else if lower_event_outcome_payout as u64 > total_collateral {
                // we can't payout more than there is in the contract, hence, we need to add this
                // check
                total_collateral
            } else {
                lower_event_outcome_payout as u64
            };

            let upper_event_outcome =
                (current_price + PAYOUT_CURVE_DISCRETIZATION_STEPS).min(BTCUSD_MAX_PRICE);
            let pnl = calculate_pnl(
                initial_price,
                Decimal::from(upper_event_outcome),
                quantity,
                offer_direction,
                long_margin,
                short_margin,
            )?;
            let upper_event_outcome_payout =
                ((offer_collateral as i64 + pnl) as u64).min(total_collateral);
            Ok((
                PayoutPoint {
                    event_outcome: lower_event_outcome.to_u64().expect("to fit into u64"),
                    outcome_payout: (lower_event_outcome_payout + fee)
                        .min(total_collateral)
                        .max(0),
                    extra_precision: 0,
                },
                PayoutPoint {
                    event_outcome: upper_event_outcome,
                    outcome_payout: (upper_event_outcome_payout + fee)
                        .min(total_collateral)
                        .max(0),
                    extra_precision: 0,
                },
            ))
        })
        .collect::<Result<Vec<(_, _)>>>()?;

    Ok(pieces)
}

/// Calculates the payout points between 0 and the lower liquidation point
fn calculate_short_liquidation_interval_payouts(
    offer_direction: Direction,
    total_collateral: u64,
    liquidation_price_lower_bound: Decimal,
    fee: u64,
) -> Result<(PayoutPoint, PayoutPoint)> {
    let (lower, upper) = match offer_direction {
        // if offerer is short, he gets everything from 0 until the acceptor's liquidation point
        Direction::Short => (
            PayoutPoint {
                event_outcome: 0,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: liquidation_price_lower_bound
                    .to_u64()
                    .expect("to be able to fit decimal into u64"),
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
        ),
        // if offerer is long, he gets the fee from 0 until his liquidation point
        Direction::Long => (
            PayoutPoint {
                event_outcome: 0,
                outcome_payout: fee,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: liquidation_price_lower_bound
                    .to_u64()
                    .expect("to be able to fit decimal into u64"),
                outcome_payout: fee,
                extra_precision: 0,
            },
        ),
    };

    Ok((lower, upper))
}

/// Calculates the upper range payout points between upper liquidation point and BTCUSD_MAX_PRICE
fn calculate_upper_range_payouts(
    offer_direction: Direction,
    total_collateral: u64,
    last_payout_point: PayoutPoint,
    fee: u64,
) -> Result<(PayoutPoint, PayoutPoint)> {
    let (lower_range_lower, lower_range_upper) = match offer_direction {
        // if offerer is long, he gets everything from the acceptor's liquidation point to
        // infinity
        Direction::Long => (
            PayoutPoint {
                event_outcome: last_payout_point.event_outcome,
                outcome_payout: (last_payout_point.outcome_payout + fee).min(total_collateral),
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE
                    .to_u64()
                    .expect("to be able to fit decimal into u64"),
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
        ),
        // if offerer is short, he gets 0 from his liquidation point to infinity
        Direction::Short => (
            PayoutPoint {
                event_outcome: last_payout_point.event_outcome,
                outcome_payout: last_payout_point.outcome_payout,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                outcome_payout: 0,
                extra_precision: 0,
            },
        ),
    };

    Ok((lower_range_lower, lower_range_upper))
}

#[cfg(test)]
mod tests {
    use crate::calculate_mid_range_payouts;
    use crate::calculate_short_liquidation_interval_payouts;
    use crate::calculate_upper_range_payouts;
    use crate::PayoutPoint;
    use anyhow::Result;
    use bitcoin::Amount;
    use proptest::prelude::*;
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use serde::Deserialize;
    use serde::Serialize;
    use std::fs::File;
    use std::ops::Mul;
    use trade::cfd::calculate_long_liquidation_price;
    use trade::cfd::calculate_margin;
    use trade::cfd::calculate_short_liquidation_price;
    use trade::cfd::BTCUSD_MAX_PRICE;
    use trade::Direction;

    /// set this to true to export test data to csv files
    /// An example gnuplot file has been provided in [`payout_curve.gp`]
    const PRINT_CSV: bool = false;

    #[test]
    fn calculate_lower_range_payout_points_when_offerer_long_then_gets_zero() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        let fee = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) =
            calculate_short_liquidation_interval_payouts(
                Direction::Long,
                total_collateral,
                bound,
                fee,
            )
            .unwrap();

        // assert
        assert_eq!(lower_payout_lower.event_outcome, 0);
        assert_eq!(lower_payout_lower.outcome_payout, fee);
        assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
        assert_eq!(lower_payout_upper.outcome_payout, fee);

        if PRINT_CSV {
            let file = File::create("src/payout_curve/lower_range_long.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            wtr.serialize(lower_payout_lower)
                .expect("to be able to write");
            wtr.serialize(lower_payout_upper)
                .expect("to be able to write");
            wtr.flush().unwrap();
        }
    }

    #[test]
    fn calculate_lower_range_payout_points_when_offerer_long_then_gets_zero_plus_fee() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        // 0.003 BTC
        let fee = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) =
            calculate_short_liquidation_interval_payouts(
                Direction::Long,
                total_collateral,
                bound,
                fee,
            )
            .unwrap();

        // assert
        assert_eq!(lower_payout_lower.event_outcome, 0);
        assert_eq!(lower_payout_lower.outcome_payout, fee);
        assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
        assert_eq!(lower_payout_upper.outcome_payout, fee);

        if PRINT_CSV {
            let file = File::create("src/payout_curve/lower_range_long.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            wtr.serialize(lower_payout_lower)
                .expect("to be able to write");
            wtr.serialize(lower_payout_upper)
                .expect("to be able to write");
            wtr.flush().unwrap();
        }
    }

    #[test]
    fn calculate_lower_range_payout_points_when_offer_short_then_gets_all() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        let fee = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) =
            calculate_short_liquidation_interval_payouts(
                Direction::Short,
                total_collateral,
                bound,
                fee,
            )
            .unwrap();

        // assert
        assert_eq!(lower_payout_lower.event_outcome, 0);
        assert_eq!(lower_payout_lower.outcome_payout, total_collateral);
        assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
        assert_eq!(lower_payout_upper.outcome_payout, total_collateral);

        // print to csv
        if PRINT_CSV {
            let file = File::create("src/payout_curve/lower_range_short.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            wtr.serialize(lower_payout_lower)
                .expect("to be able to write");
            wtr.serialize(lower_payout_upper)
                .expect("to be able to write");
            wtr.flush().unwrap();
        }
    }

    #[test]
    fn snapshot_test_mid_range_offerer() {
        // setup
        let long_leverage = 2.0;
        let short_leverage = 2.0;
        let initial_price = dec!(30_000);
        let quantity = 60_000.0;
        let fee = 300_000;
        let accept_collateral = calculate_margin(initial_price, quantity, short_leverage);
        let offer_collateral = calculate_margin(initial_price, quantity, long_leverage);

        let short_liquidation_price = calculate_short_liquidation_price(
            Decimal::from_f32(short_leverage).expect("to fit into f32"),
            initial_price,
        );
        let long_liquidation_price = calculate_long_liquidation_price(
            Decimal::from_f32(long_leverage).expect("to fit into f32"),
            initial_price,
        );

        let lower_limit = long_liquidation_price.to_u64().expect("to fit into u64");
        let upper_limit = short_liquidation_price.to_u64().expect("to fit into u64");

        let should_mid_range_payouts =
            should_data_offerer().expect("To be able to load sample data");

        // act: offer long
        let mid_range_payouts_offer_long = calculate_mid_range_payouts(
            accept_collateral,
            offer_collateral,
            initial_price,
            lower_limit,
            upper_limit,
            &PayoutPoint {
                event_outcome: lower_limit,
                outcome_payout: fee,
                extra_precision: 0,
            },
            Direction::Long,
            quantity,
            fee,
        )
        .expect("To be able to compute mid range");

        // act: offer short
        let mid_range_payouts_offer_short = calculate_mid_range_payouts(
            accept_collateral,
            offer_collateral,
            initial_price,
            lower_limit,
            upper_limit,
            &PayoutPoint {
                event_outcome: lower_limit,
                outcome_payout: offer_collateral + accept_collateral,
                extra_precision: 0,
            },
            Direction::Short,
            quantity,
            fee,
        )
        .expect("To be able to compute mid range");

        if PRINT_CSV {
            let file = File::create("src/payout_curve/mid_range_long.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            for (lower, upper) in &mid_range_payouts_offer_long {
                wtr.serialize(lower).expect("to be able to write");
                wtr.serialize(upper).expect("to be able to write");
            }
            wtr.flush().unwrap();
            let file = File::create("src/payout_curve/mid_range_short.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            for (lower, upper) in &mid_range_payouts_offer_short {
                wtr.serialize(lower).expect("to be able to write");
                wtr.serialize(upper).expect("to be able to write");
            }
            wtr.flush().unwrap();
        }

        // assert
        for (lower, upper) in &mid_range_payouts_offer_short {
            assert!(
                should_mid_range_payouts
                    .iter()
                    .any(|item| item.start == lower.event_outcome
                        && (item.payout_offer + item.fee)
                            .min(offer_collateral + accept_collateral)
                            == lower.outcome_payout),
                "{:?} was not in should payout curve - offer",
                lower
            );
            assert!(
                should_mid_range_payouts
                    .iter()
                    .any(|item| item.start == upper.event_outcome
                        && (item.payout_offer + item.fee)
                            .min(offer_collateral + accept_collateral)
                            == upper.outcome_payout),
                "{:?} was not in should payout curve - offer",
                upper
            );
        }
        for (lower, upper) in &mid_range_payouts_offer_long {
            assert!(
                should_mid_range_payouts
                    .iter()
                    .any(|item| item.start == lower.event_outcome
                        && (item.payout_accept + item.fee)
                            .min(offer_collateral + accept_collateral)
                            == lower.outcome_payout),
                "{:?} was not in should payout curve - accept",
                lower
            );
            assert!(
                should_mid_range_payouts
                    .iter()
                    .any(|item| item.start == upper.event_outcome
                        && (item.payout_accept + item.fee)
                            .min(offer_collateral + accept_collateral)
                            == upper.outcome_payout),
                "{:?} was not in should payout curve - accept",
                upper
            );
        }
    }

    #[test]
    fn ensure_all_bounds_smaller_or_equal_max_btc_price() {
        // setup
        let long_leverage = 2.0;
        let short_leverage = 1.0;
        let initial_price = dec!(36780);
        let quantity = 19.0;
        let fee = 155;
        let accept_collateral = calculate_margin(initial_price, quantity, short_leverage);
        let offer_collateral = calculate_margin(initial_price, quantity, long_leverage);

        let short_liquidation_price = calculate_short_liquidation_price(
            Decimal::from_f32(short_leverage).expect("to fit into f32"),
            initial_price,
        );
        let long_liquidation_price = calculate_long_liquidation_price(
            Decimal::from_f32(long_leverage).expect("to fit into f32"),
            initial_price,
        );

        let lower_limit = long_liquidation_price.to_u64().expect("to fit into u64");
        let upper_limit = short_liquidation_price.to_u64().expect("to fit into u64");

        // act: offer long
        let mid_range_payouts_offer_long = calculate_mid_range_payouts(
            accept_collateral,
            offer_collateral,
            initial_price,
            lower_limit,
            upper_limit,
            &PayoutPoint {
                event_outcome: lower_limit,
                outcome_payout: fee,
                extra_precision: 0,
            },
            Direction::Long,
            quantity,
            fee,
        )
        .expect("To be able to compute mid range");

        for (lower, upper) in &mid_range_payouts_offer_long {
            assert!(
                lower.event_outcome <= BTCUSD_MAX_PRICE,
                "{} > {}",
                lower.event_outcome,
                BTCUSD_MAX_PRICE
            );
            assert!(
                upper.event_outcome <= BTCUSD_MAX_PRICE,
                "{} > {}",
                upper.event_outcome,
                BTCUSD_MAX_PRICE
            );
        }
    }

    #[test]
    fn calculate_upper_range_payout_points_when_offer_short_then_gets_zero() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let last_payout = PayoutPoint {
            event_outcome: 60_000,
            outcome_payout: 0,
            extra_precision: 0,
        };
        let fee = 300_000;
        // act
        let (lower, upper) = calculate_upper_range_payouts(
            Direction::Short,
            total_collateral,
            last_payout.clone(),
            fee,
        )
        .unwrap();

        // assert
        assert_eq!(lower.event_outcome, last_payout.event_outcome);
        assert_eq!(lower.outcome_payout, 0);
        assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
        assert_eq!(upper.outcome_payout, 0);

        if PRINT_CSV {
            let file = File::create("src/payout_curve/upper_range_short.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            wtr.serialize(lower).expect("to be able to write");
            wtr.serialize(upper).expect("to be able to write");
            wtr.flush().unwrap();
        }
    }

    #[test]
    fn calculate_upper_range_payout_points_when_offer_long_then_gets_everything() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let last_payout = PayoutPoint {
            event_outcome: 60_000,
            outcome_payout: total_collateral,
            extra_precision: 0,
        };
        let fee = 300_000;

        // act
        let (lower, upper) = calculate_upper_range_payouts(
            Direction::Long,
            total_collateral,
            last_payout.clone(),
            fee,
        )
        .unwrap();

        // assert
        assert_eq!(lower.event_outcome, last_payout.event_outcome);
        assert_eq!(lower.outcome_payout, total_collateral);
        assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
        assert_eq!(upper.outcome_payout, total_collateral);

        if PRINT_CSV {
            let file = File::create("src/payout_curve/upper_range_long.csv").unwrap();
            let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
            wtr.serialize(lower).expect("to be able to write");
            wtr.serialize(upper).expect("to be able to write");
            wtr.flush().unwrap();
        }
    }

    #[test]
    fn upper_range_price_always_below_max_btc_price() {
        // setup
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let last_payout = PayoutPoint {
            event_outcome: BTCUSD_MAX_PRICE,
            outcome_payout: total_collateral,
            extra_precision: 0,
        };
        let fee = 300_000;

        // act
        let (lower, upper) = calculate_upper_range_payouts(
            Direction::Long,
            total_collateral,
            last_payout.clone(),
            fee,
        )
        .unwrap();

        // assert
        assert_eq!(lower.event_outcome, last_payout.event_outcome);
        assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
    }

    /// Loads the sample data from a csv file
    fn should_data_offerer() -> Result<Vec<ShouldPayout>> {
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b';')
            .from_path("src/payout_curve/should_data_offer_short.csv")?;
        let mut should_samples = vec![];
        for result in rdr.deserialize() {
            let record: ShouldPayout = result?;

            should_samples.push(record);
        }
        Ok(should_samples)
    }

    #[derive(Serialize, Deserialize)]
    struct PayoutCouple {
        lower_event_outcome: u64,
        lower_outcome_payout: u64,
        lower_extra_precision: u16,
        upper_event_outcome: u64,
        upper_outcome_payout: u64,
        upper_extra_precision: u16,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct ShouldPayout {
        start: u64,
        payout_offer: u64,
        payout_accept: u64,
        fee: u64,
    }

    //******* Proptests *******//

    proptest! {

        #[test]
        fn calculating_lower_bound_doesnt_crash_offer_short(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let bound = Decimal::from_u64(bound).expect("to be able to parse bound");
            let fee = 300_000;

            // act:
            let (lower_payout_lower, lower_payout_upper) =
                calculate_short_liquidation_interval_payouts(Direction::Short, total_collateral, bound, fee).unwrap();

            // assert
            prop_assert_eq!(lower_payout_lower.event_outcome, 0);
            prop_assert_eq!(lower_payout_lower.outcome_payout, total_collateral);
            prop_assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
            prop_assert_eq!(lower_payout_upper.outcome_payout, total_collateral);
        }
    }

    proptest! {

        #[test]
        fn calculating_lower_bound_doesnt_crash_offer_long(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let bound = Decimal::from_u64(bound).expect("to be able to parse bound");
            let fee = 300_000;

            // act:
            let (lower_payout_lower, lower_payout_upper) =
                calculate_short_liquidation_interval_payouts(Direction::Short, total_collateral, bound, fee).unwrap();

            // assert
            prop_assert_eq!(lower_payout_lower.event_outcome, 0);
            prop_assert_eq!(lower_payout_lower.outcome_payout, total_collateral);
            prop_assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
            prop_assert_eq!(lower_payout_upper.outcome_payout, total_collateral);
        }
    }

    proptest! {

        #[test]
        fn calculating_upper_bound_doesnt_crash_offer_short(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let last_payout = PayoutPoint {
                event_outcome: bound,
                outcome_payout: total_collateral,
                extra_precision: 0,
            };
            let fee = 300_000;

            // act
            let (lower, upper) =
                calculate_upper_range_payouts(Direction::Short, total_collateral, last_payout.clone(), fee).unwrap();

            // assert
            prop_assert_eq!(lower.event_outcome, last_payout.event_outcome);
            prop_assert_eq!(lower.outcome_payout, last_payout.outcome_payout);
            prop_assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
            prop_assert_eq!(upper.outcome_payout, 0);
        }

    }

    proptest! {

        #[test]
        fn calculating_upper_bound_doesnt_crash_offer_long(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let last_payout = PayoutPoint {
                event_outcome: bound,
                outcome_payout: total_collateral,
                extra_precision: 0,
            };
            let fee = 300_000;
            // act
            let (lower, upper) =
                calculate_upper_range_payouts(Direction::Long, total_collateral, last_payout.clone(), fee).unwrap();

            // assert
            assert_eq!(lower.event_outcome, last_payout.event_outcome);
            assert_eq!(lower.outcome_payout, total_collateral);
            assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
            assert_eq!(upper.outcome_payout, total_collateral);
        }
    }

    proptest! {

        #[test]
        fn midrange_always_positive(initial_price in 20_000i32..50_000, short_leverage in 1i32..5) {
            // setup
            let long_leverage = 2.0;
            let short_leverage = short_leverage as f32;
            let initial_price = Decimal::from_i32(initial_price).expect("to be able to parse");
            let quantity = 1000.0;
            let fee = dec!(0.003) * Decimal::from_f32(quantity).expect("to be able to parse into dec")
                / initial_price;
            let fee = fee
                .mul(dec!(100_000_000))
                .to_u64()
                .expect("to fit into u64");

            let accept_collateral = calculate_margin(initial_price, quantity, short_leverage);
            let offer_collateral = calculate_margin(initial_price, quantity, long_leverage);

            let short_liquidation_price = calculate_short_liquidation_price(
                Decimal::from_f32(short_leverage).expect("to fit into f32"),
                initial_price,
            );
            let long_liquidation_price = calculate_long_liquidation_price(
                Decimal::from_f32(long_leverage).expect("to fit into f32"),
                initial_price,
            );

            let lower_limit = long_liquidation_price.to_u64().expect("to fit into u64");
            let upper_limit = short_liquidation_price.to_u64().expect("to fit into u64");

            // act: offer long
            let mid_range_payouts_offer_long = calculate_mid_range_payouts(
                accept_collateral,
                offer_collateral,
                initial_price,
                lower_limit,
                upper_limit,
                &PayoutPoint {
                    event_outcome: lower_limit,
                    outcome_payout: fee,
                    extra_precision: 0,
                },
                Direction::Long,
                quantity,
                fee,
            )
            .expect("To be able to compute mid range");

            // assert
            mid_range_payouts_offer_long
                .iter()
                .all(|(lower, upper)| lower.outcome_payout > 0 && upper.outcome_payout > 0);
        }

    }
}
