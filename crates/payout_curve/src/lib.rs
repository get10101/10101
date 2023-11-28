use anyhow::Context;
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_pnl;
use trade::cfd::calculate_short_liquidation_price;
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
const PAYOUT_CURVE_DISCRETIZATION_STEPS: u64 = 1;

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
/// that of the offer party, i.e. the coordinator in 10101's case.
///
/// Returns a `Vec<(PayoutPoint, PayoutPoint)>>`. Each tuple maps to one
/// [`dlc_manager::payout_curve::PolynomialPayoutCurvePiece`].
///
/// The `fee` is always paid towards the offer party.
#[allow(clippy::too_many_arguments)]
pub fn build_inverse_payout_function(
    quantity: f32,
    offer_collateral: u64,
    accept_collateral: u64,
    initial_price: Decimal,
    accept_leverage: f32,
    offer_leverage: f32,
    fee: u64,
    offer_direction: Direction,
) -> Result<Vec<(PayoutPoint, PayoutPoint)>> {
    let mut pieces = vec![];
    let total_collateral = offer_collateral + accept_collateral;

    // Calculate the long and short liquidation prices, depending on the direction of the offer
    // party.
    let (long_liquidation_threshold, short_liquidation_threshold) = calculate_payout_curve_bounds(
        initial_price,
        accept_leverage,
        offer_leverage,
        offer_direction,
    )?;

    let (short_leverage, long_leverage) = {
        if offer_direction == Direction::Long {
            (accept_leverage, offer_leverage)
        } else {
            (offer_leverage, accept_leverage)
        }
    };

    // The long liquidation interval corresponds to _low_ prices.
    let (long_liquidation_interval_lower_bound, long_liquidation_interval_upper_bound) =
        calculate_long_liquidation_interval_payouts(
            offer_direction,
            total_collateral,
            long_liquidation_threshold,
            fee,
        )?;

    let long_liquidation_threshold_payout = long_liquidation_interval_upper_bound.clone();

    pieces.push((
        long_liquidation_interval_lower_bound,
        long_liquidation_interval_upper_bound,
    ));

    let mid_range = calculate_mid_range_payouts(
        accept_collateral,
        offer_collateral,
        long_leverage,
        short_leverage,
        initial_price,
        long_liquidation_threshold
            .to_u64()
            .expect("to fit dec into u64"),
        short_liquidation_threshold
            .to_u64()
            .expect("to fit dec into u64"),
        &long_liquidation_threshold_payout,
        offer_direction,
        quantity,
        fee,
    )?;

    let (_, last_mid_range) = mid_range
        .last()
        .context("didn't have at least a signel element in the mid range")?
        .clone();

    for (lower, upper) in mid_range {
        pieces.push((lower, upper));
    }
    // if the upper bound is already [`BTCUSD_MAX_PRICE`] we don't have to add the upper bound
    // anymore
    if last_mid_range.event_outcome < BTCUSD_MAX_PRICE {
        let short_liquidation_interval_payouts = calculate_short_liquidation_interval_payouts(
            offer_direction,
            total_collateral,
            last_mid_range,
            fee,
        )?;
        pieces.push(short_liquidation_interval_payouts);
    }

    Ok(pieces)
}

/// Calculates the mid-range [`PayoutPoint`]s between the long and short liquidation thresholds.
///
/// Returns a vector of tuples of [`PayoutPoint`]s, with the elements in the vector and in the
/// tuples being ordered from low to high price.
#[allow(clippy::too_many_arguments)]
fn calculate_mid_range_payouts(
    accept_collateral: u64,
    offer_collateral: u64,
    long_leverage: f32,
    short_leverage: f32,
    initial_price: Decimal,
    lower_limit: u64,
    upper_limit: u64,
    long_liquidation_threshold_payout: &PayoutPoint,
    direction: Direction,
    quantity: f32,
    fee: u64,
) -> Result<Vec<(PayoutPoint, PayoutPoint)>> {
    let total_collateral = accept_collateral + offer_collateral;
    let long_leverage = long_leverage.to_f32().expect("to fit into f32");
    let short_leverage = short_leverage.to_f32().expect("to fit into f32");

    let pieces = (lower_limit..upper_limit)
        .step_by(PAYOUT_CURVE_DISCRETIZATION_STEPS as usize)
        .map(|low_event_outcome| {
            let lower_event_outcome_payout = if low_event_outcome == lower_limit {
                // the last_payout includes already the fee. Hence, we need to subtract it here as
                // we add it again later
                (long_liquidation_threshold_payout.outcome_payout - fee) as i64
            } else {
                offer_collateral as i64
                    + calculate_pnl(
                        initial_price,
                        Decimal::from(low_event_outcome),
                        quantity,
                        long_leverage,
                        short_leverage,
                        direction,
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
                (low_event_outcome + PAYOUT_CURVE_DISCRETIZATION_STEPS).min(BTCUSD_MAX_PRICE);
            let pnl = calculate_pnl(
                initial_price,
                Decimal::from(upper_event_outcome),
                quantity,
                long_leverage,
                short_leverage,
                direction,
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

/// Calculates the payout points between 0 and the long liquidation point i.e. that point at which
/// the party going long gets liquidated or gets nothing other than the fee if applicable.
fn calculate_long_liquidation_interval_payouts(
    offer_direction: Direction,
    total_collateral: u64,
    long_liquidation_threshold: Decimal,
    fee: u64,
) -> Result<(PayoutPoint, PayoutPoint)> {
    let (lower, upper) = match offer_direction {
        // If the offer party is going short, they get everything between 0 and the
        // `long_liquidation_threshold`.
        Direction::Short => (
            PayoutPoint {
                event_outcome: 0,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: long_liquidation_threshold
                    .to_u64()
                    .expect("to be able to fit decimal into u64"),
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
        ),
        // If the offer party is going long, they just get the `fee` between 0 and the
        // `long_liquidation_threshold`.
        Direction::Long => (
            PayoutPoint {
                event_outcome: 0,
                outcome_payout: fee,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: long_liquidation_threshold
                    .to_u64()
                    .expect("to be able to fit decimal into u64"),
                outcome_payout: fee,
                extra_precision: 0,
            },
        ),
    };

    Ok((lower, upper))
}

/// Calculates the payout points between [`BTCUSD_MAX_PRICE`] and the short liquidation point i.e.
/// that point at which the party going short gets liquidated or gets nothing other than the fee if
/// applicable.
fn calculate_short_liquidation_interval_payouts(
    offer_direction: Direction,
    total_collateral: u64,
    short_liquidation_threshold: PayoutPoint,
    fee: u64,
) -> Result<(PayoutPoint, PayoutPoint)> {
    let (lower_range_lower, lower_range_upper) = match offer_direction {
        // If the offer party is going long, they get everything between the
        // `short_liquidation_threshold` and the `BTCUSD_MAX_PRICE`.
        Direction::Long => (
            PayoutPoint {
                event_outcome: short_liquidation_threshold.event_outcome,
                outcome_payout: (dbg!(short_liquidation_threshold.outcome_payout + fee))
                    .min(dbg!(total_collateral)),
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
        ),
        // If the offer party is going short, they just get nothing between the
        // `short_liquidation_threshold` and the `BTCUSD_MAX_PRICE`.
        Direction::Short => (
            PayoutPoint {
                event_outcome: short_liquidation_threshold.event_outcome,
                outcome_payout: dbg!(short_liquidation_threshold.outcome_payout),
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                // TODO: Why does the short miss out on the fee?
                outcome_payout: 0,
                extra_precision: 0,
            },
        ),
    };

    Ok((lower_range_lower, lower_range_upper))
}

/// Calculates lower and upper bounds for our payout curve, aka the liquidation prices
///
/// Note: the upper bound is bound by [`BTCUSD_MAX_PRICE`]
fn calculate_payout_curve_bounds(
    initial_price: Decimal,
    accept_leverage: f32,
    offer_leverage: f32,
    offer_direction: Direction,
) -> Result<(Decimal, Decimal)> {
    let (liquidation_price_lower_bound, liquidation_price_upper_bound) = match offer_direction {
        Direction::Long => {
            let leverage_short = Decimal::try_from(accept_leverage)?;
            let liquidation_price_short =
                calculate_short_liquidation_price(leverage_short, initial_price);

            let leverage_long = Decimal::try_from(offer_leverage)?;
            let liquidation_price_long =
                calculate_long_liquidation_price(leverage_long, initial_price);
            // if the offerer is long, his lower bound is when he gets liquidated and the upper
            // bound is when the acceptor gets liquidated
            (liquidation_price_long, liquidation_price_short)
        }
        Direction::Short => {
            let leverage_short = Decimal::try_from(offer_leverage)?;
            let liquidation_price_short =
                calculate_short_liquidation_price(leverage_short, initial_price);

            let leverage_long = Decimal::try_from(accept_leverage)?;
            let liquidation_price_long =
                calculate_long_liquidation_price(leverage_long, initial_price);
            // if the offerer is short, his lower bound is when the acceptor gets liquidated and
            // the upper bound is when the offerer gets liquidated
            (liquidation_price_long, liquidation_price_short)
        }
    };
    Ok((
        liquidation_price_lower_bound,
        liquidation_price_upper_bound.min(Decimal::from(BTCUSD_MAX_PRICE)),
    ))
}

#[cfg(test)]
mod tests {
    use crate::calculate_long_liquidation_interval_payouts;
    use crate::calculate_mid_range_payouts;
    use crate::calculate_payout_curve_bounds;
    use crate::calculate_short_liquidation_interval_payouts;
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
    pub fn calculate_bounds_when_offerer_long_with_different_leverages() {
        let initial_price = dec!(30_000);
        let accept_leverage = 4.0;
        let offer_leverage = 2.0;

        let (lower_bound, upper_bound) = calculate_payout_curve_bounds(
            initial_price,
            accept_leverage,
            offer_leverage,
            Direction::Long,
        )
        .unwrap();

        // offerer with leverage 2 gets liquidated
        assert_eq!(lower_bound, dec!(20_000));
        // acceptor with leverage 4 gets liquidated
        assert_eq!(upper_bound, dec!(40_000));
    }

    #[test]
    pub fn calculate_bounds_when_offerer_short_with_different_leverages() {
        let initial_price = dec!(30_000);
        let accept_leverage = 4.0;
        let offer_leverage = 2.0;

        let (lower_bound, upper_bound) = calculate_payout_curve_bounds(
            initial_price,
            accept_leverage,
            offer_leverage,
            Direction::Short,
        )
        .unwrap();

        // acceptor with leverage 4 gets liquidated
        assert_eq!(lower_bound, dec!(24_000));
        // offerer with leverage 2 gets liquidated
        assert_eq!(upper_bound, dec!(60_000));
    }

    #[test]
    pub fn calculate_bounds_when_offerer_long_with_same_leverages() {
        let initial_price = dec!(30_000);
        let accept_leverage = 2.0;
        let offer_leverage = 2.0;

        let (lower_bound, upper_bound) = calculate_payout_curve_bounds(
            initial_price,
            accept_leverage,
            offer_leverage,
            Direction::Long,
        )
        .unwrap();

        // offerer with leverage 2 gets liquidated
        assert_eq!(lower_bound, dec!(20_000));
        // acceptor with leverage 2 gets liquidated
        assert_eq!(upper_bound, dec!(60_000));
    }
    #[test]
    pub fn calculate_bounds_when_offerer_short_with_same_leverages() {
        let initial_price = dec!(30_000);
        let accept_leverage = 2.0;
        let offer_leverage = 2.0;

        let (lower_bound, upper_bound) = calculate_payout_curve_bounds(
            initial_price,
            accept_leverage,
            offer_leverage,
            Direction::Short,
        )
        .unwrap();

        // acceptor with leverage 2 gets liquidated
        assert_eq!(lower_bound, dec!(20_000));
        // offerer with leverage 2 gets liquidated
        assert_eq!(upper_bound, dec!(60_000));
    }

    #[test]
    pub fn calculate_lower_range_payout_points_when_offerer_long_then_gets_zero() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        let fee = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) = calculate_long_liquidation_interval_payouts(
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
    pub fn calculate_lower_range_payout_points_when_offerer_long_then_gets_zero_plus_fee() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        // 0.003 BTC
        let fee = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) = calculate_long_liquidation_interval_payouts(
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
    pub fn calculate_lower_range_payout_points_when_offer_short_then_gets_all() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        let fee = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) = calculate_long_liquidation_interval_payouts(
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
    pub fn snapshot_test_mid_range_offerer() {
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
            long_leverage,
            short_leverage,
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
            long_leverage,
            short_leverage,
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
    pub fn ensure_all_bounds_smaller_or_equal_max_btc_price() {
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
            long_leverage,
            short_leverage,
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
    pub fn calculate_upper_range_payout_points_when_offer_short_then_gets_zero() {
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
        let (lower, upper) = calculate_short_liquidation_interval_payouts(
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
    pub fn calculate_upper_range_payout_points_when_offer_long_then_gets_everything() {
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
        let (lower, upper) = calculate_short_liquidation_interval_payouts(
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
    pub fn upper_range_price_always_below_max_btc_price() {
        // setup
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let last_payout = PayoutPoint {
            event_outcome: BTCUSD_MAX_PRICE,
            outcome_payout: total_collateral,
            extra_precision: 0,
        };
        let fee = 300_000;

        // act
        let (lower, upper) = calculate_short_liquidation_interval_payouts(
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
        pub lower_event_outcome: u64,
        pub lower_outcome_payout: u64,
        pub lower_extra_precision: u16,
        pub upper_event_outcome: u64,
        pub upper_outcome_payout: u64,
        pub upper_extra_precision: u16,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct ShouldPayout {
        pub start: u64,
        pub payout_offer: u64,
        pub payout_accept: u64,
        pub fee: u64,
    }

    //******* Proptests *******//

    proptest! {

        #[test]
        fn calculating_lower_bound_doesnt_crash_offer_short(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let total_collateral = total_collateral;
            let bound = Decimal::from_u64(bound).expect("to be able to parse bound");
            let fee = 300_000;

            // act:
            let (lower_payout_lower, lower_payout_upper) =
                calculate_long_liquidation_interval_payouts(Direction::Short, total_collateral, bound, fee).unwrap();

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
            let total_collateral = total_collateral;
            let bound = Decimal::from_u64(bound).expect("to be able to parse bound");
            let fee = 300_000;

            // act:
            let (lower_payout_lower, lower_payout_upper) =
                calculate_long_liquidation_interval_payouts(Direction::Short, total_collateral, bound, fee).unwrap();

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
            let total_collateral = total_collateral;
            let last_payout = PayoutPoint {
                event_outcome: bound,
                outcome_payout: total_collateral,
                extra_precision: 0,
            };
            let fee = 300_000;

            // act
            let (lower, upper) =
                calculate_short_liquidation_interval_payouts(Direction::Short, total_collateral, last_payout.clone(), fee).unwrap();

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
            let total_collateral = total_collateral;
            let last_payout = PayoutPoint {
                event_outcome: bound,
                outcome_payout: total_collateral,
                extra_precision: 0,
            };
            let fee = 300_000;
            // act
            let (lower, upper) =
                calculate_short_liquidation_interval_payouts(Direction::Long, total_collateral, last_payout.clone(), fee).unwrap();

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
                long_leverage,
                short_leverage,
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
