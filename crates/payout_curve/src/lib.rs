use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use trade::cfd::calculate_pnl;
use trade::cfd::BTCUSD_MAX_PRICE;
use trade::Direction;

/// Factor by which we can multiply the total margin being wagered in order to get consistent
/// rounding in the middle (non-constant) part of the payout function.
///
/// E.g. with a value of 0.01 and a total margin of 20_000 sats would get payout jumps of 200 sats,
/// for a total of ~100 intervals.
///
/// TODO: We should not use the same rounding for all non-constant parts of the payout function,
/// because not all intervals are equally as likely. That way we can avoid excessive CET generation.
pub const ROUNDING_PERCENT: f32 = 0.01;

/// Number of intervals which we want to use to discretize the payout function.
const PAYOUT_CURVE_DISCRETIZATION_INTERVALS: u64 = 200;

/// A payout point representing a payout for a given outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PayoutPoint {
    /// The event outcome.
    pub event_outcome: u64,
    /// The payout for the outcome.
    pub outcome_payout: u64,
    /// Extra precision to use when computing the payout.
    pub extra_precision: u16,
}

#[derive(Clone, Copy)]
pub struct PartyParams {
    /// How many coins the party is wagering.
    margin: u64,
    /// How many coins the party is excluding from the bet, in sats.
    ///
    /// If the party gets liquidated, they get back exactly this much, in sats.
    collateral_reserve: u64,
}

impl PartyParams {
    pub fn new(margin: Amount, collateral_reserve: Amount) -> Self {
        Self {
            margin: margin.to_sat(),
            collateral_reserve: collateral_reserve.to_sat(),
        }
    }

    pub fn margin(&self) -> u64 {
        self.margin
    }

    /// The sum of all the coins that the party is wagering and reserving, in sats.
    ///
    /// The separation between margin and collateral may seem superfluous, but it is necessary
    /// because this code is used in a DLC channel where all of the coins are stored in a DLC
    /// output, but where all the coins are not always meant to be at stake.
    pub fn total_collateral(&self) -> u64 {
        self.margin + self.collateral_reserve
    }
}

#[derive(Clone, Copy)]
pub struct PriceParams {
    initial_price: Decimal,
    /// The price at which the party going long gets liquidated.
    ///
    /// This is _lower_ than the initial price.
    long_liquidation_price: Decimal,
    /// The price at which the party going short gets liquidated.
    ///
    /// This is _higher_ than the initial price.
    short_liquidation_price: Decimal,
}

impl PriceParams {
    pub fn new_btc_usd(
        initial: Decimal,
        long_liquidation: Decimal,
        short_liquidation: Decimal,
    ) -> Result<Self> {
        // We cap the short liquidation at the maximum possible value of Bitcoin w.r.t to USD that
        // we support.
        let short_liquidation = short_liquidation.min(Decimal::from(BTCUSD_MAX_PRICE));

        Self::new(initial, short_liquidation, long_liquidation)
    }

    fn new(
        initial: Decimal,
        short_liquidation: Decimal,
        long_liquidation: Decimal,
    ) -> Result<Self> {
        ensure!(
            long_liquidation <= initial,
            "Long liquidation price should not be greater than the initial price"
        );

        ensure!(
            initial <= short_liquidation,
            "Short liquidation price should not be smaller than the initial price"
        );

        Ok(Self {
            initial_price: initial,
            short_liquidation_price: short_liquidation,
            long_liquidation_price: long_liquidation,
        })
    }
}

/// Build a discretized payout function for an inverse perpetual future (e.g. BTCUSD) from the
/// perspective of the offer party.
///
/// Returns a `Vec<(PayoutPoint, PayoutPoint)>`, with the first element of the tuple being the start
/// of the interval and the second element of the tuple being the end of the interval.
///
/// Each tuple is meant to map to one [`dlc_manager::payout_curve::PolynomialPayoutCurvePiece`] when
/// building the corresponding [`dlc_manager::payout_curve::PayoutFunction`].
pub fn build_inverse_payout_function(
    // The number of contracts.
    quantity: f32,
    offer_party: PartyParams,
    accept_party: PartyParams,
    price_params: PriceParams,
    offer_party_direction: Direction,
) -> Result<Vec<(PayoutPoint, PayoutPoint)>> {
    let mut pieces = vec![];

    let total_collateral = offer_party.total_collateral() + accept_party.total_collateral();

    let (collateral_reserve_long, collateral_reserve_short) = match offer_party_direction {
        Direction::Long => (
            offer_party.collateral_reserve,
            accept_party.collateral_reserve,
        ),
        Direction::Short => (
            accept_party.collateral_reserve,
            offer_party.collateral_reserve,
        ),
    };

    let (long_liquidation_interval_start, long_liquidation_interval_end) =
        calculate_long_liquidation_interval_payouts(
            offer_party_direction,
            total_collateral,
            price_params.long_liquidation_price,
            collateral_reserve_long,
        )?;
    pieces.push((
        long_liquidation_interval_start,
        long_liquidation_interval_end,
    ));

    let (short_liquidation_interval_start, short_liquidation_interval_end) =
        calculate_short_liquidation_interval_payouts(
            offer_party_direction,
            total_collateral,
            price_params.short_liquidation_price,
            collateral_reserve_short,
        )?;

    let mid_range = calculate_mid_range_payouts(
        offer_party,
        accept_party,
        price_params.initial_price,
        &long_liquidation_interval_end,
        &short_liquidation_interval_start,
        offer_party_direction,
        quantity,
    )?;

    for (lower, upper) in mid_range.iter() {
        pieces.push((*lower, *upper));
    }

    pieces.push((
        short_liquidation_interval_start,
        short_liquidation_interval_end,
    ));

    // Connect the intervals `[(X, A), (Y, A))]` and `[(Y, B), (Z, B)]` by introducing a step-up
    // interval in between:
    //
    // `[(X, A), (Y - 1, A))]`, `[(Y - 1, A), (Y, B)]`, `[(Y, B), (Z, D)]`.
    //
    // E.g. converting
    //
    // `[($100, 60 sats), ($200, 60 sats)]`, `[($200, 30 sats), ($300, 30 sats)]` into
    //
    // `[($100, 60 sats), ($199, 60 sats)]`, `[($199, 60 sats), ($200, 30 sats)]`, `[($200, 30
    // sats), ($300, 30 sats)]`.
    let pieces_minus_first = pieces.iter().skip(1);
    let mut pieces = pieces
        .iter()
        .zip(pieces_minus_first)
        .flat_map(|((a, b), (c, _))| {
            let shared_point = PayoutPoint {
                event_outcome: b.event_outcome - 1,
                ..*b
            };

            vec![(*a, shared_point), (shared_point, *c)]
        })
        .collect::<Vec<(PayoutPoint, PayoutPoint)>>();

    // The last interval is dropped by zipping an iterator of length `L` with an iterator of length
    // `L-1`, dropping the last element implicitly. Therefore, we need to reintroduce the last
    // element.
    pieces.push((
        short_liquidation_interval_start,
        short_liquidation_interval_end,
    ));

    let pieces = pieces
        .into_iter()
        .filter(|(start, end)| start.event_outcome != end.event_outcome)
        .collect::<Vec<_>>();

    Ok(pieces)
}

/// Calculate the payout points for the interval where the party going long gets liquidated, from
/// the perspective of the offer party.
///
/// The price ranges from 0 to the `long_liquidation_price`.
fn calculate_long_liquidation_interval_payouts(
    offer_direction: Direction,
    total_collateral: u64,
    liquidation_price_long: Decimal,
    collateral_reserve_long: u64,
) -> Result<(PayoutPoint, PayoutPoint)> {
    let liquidation_price_long = liquidation_price_long
        .to_u64()
        .expect("to be able to fit decimal into u64");

    let (lower, upper) = match offer_direction {
        // If the offer party is short and the long party gets liquidated, the offer party gets all
        // the collateral minus the long party's collateral reserve.
        Direction::Short => {
            let outcome_payout = total_collateral - collateral_reserve_long;

            (
                PayoutPoint {
                    event_outcome: 0,
                    outcome_payout,
                    extra_precision: 0,
                },
                PayoutPoint {
                    event_outcome: liquidation_price_long,
                    outcome_payout,
                    extra_precision: 0,
                },
            )
        }
        // If the offer party is long and they get liquidated, they get their collateral reserve.
        Direction::Long => (
            PayoutPoint {
                event_outcome: 0,
                outcome_payout: collateral_reserve_long,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: liquidation_price_long,
                outcome_payout: collateral_reserve_long,
                extra_precision: 0,
            },
        ),
    };

    Ok((lower, upper))
}

/// Calculates the payout points for the interval between the `long_liquidation_price` and the
/// `short_liquidation_price`.
///
/// Returns tuples of payout points, first item is lower point, next item is higher point of two
/// points on the payout curve.
fn calculate_mid_range_payouts(
    offer_party: PartyParams,
    accept_party: PartyParams,
    initial_price: Decimal,
    // The end of the price interval within which the party going long gets liquidated. This is the
    // highest of the two points in terms of price.
    long_liquidation_interval_end_payout: &PayoutPoint,
    // The start of the price interval within which the party going short gets liquidated. This is
    // the lowest of the two points in terms of price.
    short_liquidation_interval_start_payout: &PayoutPoint,
    offer_direction: Direction,
    quantity: f32,
) -> Result<Vec<(PayoutPoint, PayoutPoint)>> {
    let long_liquidation_price = long_liquidation_interval_end_payout.event_outcome;
    let short_liquidation_price = short_liquidation_interval_start_payout.event_outcome;

    let min_payout_offer_party = offer_party.collateral_reserve;

    // This excludes the collateral reserve of the accept party.
    let max_payout_offer_party = offer_party.total_collateral() + accept_party.margin;

    let (long_margin, short_margin) = match offer_direction {
        Direction::Long => (offer_party.margin, accept_party.margin),
        Direction::Short => (accept_party.margin, offer_party.margin),
    };

    let step = {
        let diff = short_liquidation_price
            .checked_sub(long_liquidation_price)
            .context("Long liquidation price smaller than short liquidation price")?;
        diff / PAYOUT_CURVE_DISCRETIZATION_INTERVALS
    };

    let pieces = (long_liquidation_price..short_liquidation_price)
        .step_by(step as usize)
        .map(|interval_start_price| {
            let interval_mid_price = interval_start_price + step / 2;

            let pnl = calculate_pnl(
                initial_price,
                Decimal::from(interval_mid_price),
                quantity,
                offer_direction,
                long_margin,
                short_margin,
            )?;

            // If this is the start of the middle interval after the long liquidation interval.
            let interval_payout = offer_party.total_collateral() as i64 + pnl;

            // Payout cannot be below min.
            let interval_payout = interval_payout.max(min_payout_offer_party as i64);

            // Payout cannot be above max.
            let interval_payout = interval_payout.min(max_payout_offer_party as i64);

            let interval_start_payout_point = PayoutPoint {
                event_outcome: interval_start_price,
                outcome_payout: interval_payout as u64,
                extra_precision: 0,
            };

            let interval_end_price = (interval_start_price + step).min(short_liquidation_price);

            let interval_end_payout_point = PayoutPoint {
                event_outcome: interval_end_price,
                outcome_payout: interval_payout as u64,
                extra_precision: 0,
            };

            Ok((interval_start_payout_point, interval_end_payout_point))
        })
        .collect::<Result<Vec<(_, _)>>>()?;

    Ok(pieces)
}

/// Calculate the payout points for the interval where the party going short gets liquidated, from
/// the perspective of the offer party.
///
/// The price ranges from the `short_liquidation_price` to `BTCUSD_MAX_PRICE`.
fn calculate_short_liquidation_interval_payouts(
    offer_direction: Direction,
    total_collateral: u64,
    liquidation_price_short: Decimal,
    collateral_reserve_short: u64,
) -> Result<(PayoutPoint, PayoutPoint)> {
    let liquidation_price_short = {
        let price = liquidation_price_short.to_u64().expect("to fit");

        // We cannot end up generating an interval with a constant price, because `rust-dlc` says
        // that `Payout points must have ascending event outcome value`.
        if price == BTCUSD_MAX_PRICE {
            price - 1
        } else {
            price
        }
    };

    let (lower, upper) = match offer_direction {
        // If the offer party is long and the short party gets liquidated, the offer party gets all
        // the collateral minus the short party's collateral reserve.
        Direction::Long => {
            let outcome_payout = total_collateral - collateral_reserve_short;

            let interval_start = PayoutPoint {
                event_outcome: liquidation_price_short,
                outcome_payout,
                extra_precision: 0,
            };

            let interval_end = PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                outcome_payout,
                extra_precision: 0,
            };

            (interval_start, interval_end)
        }
        // If the offer party is short and they get liquidated, they get their collateral reserve.
        Direction::Short => {
            let outcome_payout = collateral_reserve_short;

            let interval_start = PayoutPoint {
                event_outcome: liquidation_price_short,
                outcome_payout,
                extra_precision: 0,
            };

            let interval_end = PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                outcome_payout,
                extra_precision: 0,
            };

            (interval_start, interval_end)
        }
    };

    Ok((lower, upper))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use proptest::prelude::*;
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal_macros::dec;
    use std::fs::File;
    use std::ops::Mul;
    use trade::cfd::calculate_long_bankruptcy_price;
    use trade::cfd::calculate_margin;
    use trade::cfd::calculate_short_bankruptcy_price;

    /// set this to true to export test data to csv files
    /// An example gnuplot file has been provided in [`payout_curve.gp`]
    const PRINT_CSV: bool = false;

    #[test]
    fn calculate_lower_range_payout_points_when_offerer_long_then_gets_zero() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        let collateral_reserve_long = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) = calculate_long_liquidation_interval_payouts(
            Direction::Long,
            total_collateral,
            bound,
            collateral_reserve_long,
        )
        .unwrap();

        // assert
        assert_eq!(lower_payout_lower.event_outcome, 0);
        assert_eq!(lower_payout_lower.outcome_payout, collateral_reserve_long);
        assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
        assert_eq!(lower_payout_upper.outcome_payout, collateral_reserve_long);

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
    fn calculate_lower_range_payout_points_when_offerer_long_then_gets_zero_plus_reserve() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let bound = dec!(20_000);
        // 0.003 BTC
        let collateral_reserve_long = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) = calculate_long_liquidation_interval_payouts(
            Direction::Long,
            total_collateral,
            bound,
            collateral_reserve_long,
        )
        .unwrap();

        // assert
        assert_eq!(lower_payout_lower.event_outcome, 0);
        assert_eq!(lower_payout_lower.outcome_payout, collateral_reserve_long);
        assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
        assert_eq!(lower_payout_upper.outcome_payout, collateral_reserve_long);

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
        let collateral_reserve_long = 300_000;

        // act
        let (lower_payout_lower, lower_payout_upper) = calculate_long_liquidation_interval_payouts(
            Direction::Short,
            total_collateral,
            bound,
            collateral_reserve_long,
        )
        .unwrap();

        // assert
        assert_eq!(lower_payout_lower.event_outcome, 0);
        assert_eq!(
            lower_payout_lower.outcome_payout,
            total_collateral - collateral_reserve_long
        );
        assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
        assert_eq!(
            lower_payout_upper.outcome_payout,
            total_collateral - collateral_reserve_long
        );

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
    fn payout_function_snapshot() {
        let quantity = 60_000.0;
        let initial_price = dec!(30_000);
        let leverage_long = Decimal::TWO;
        let leverage_short = Decimal::TWO;

        let collateral_reserve_offer = Amount::from_sat(300_000);
        let collateral_reserve_accept = Amount::ZERO;

        let offer_party_direction = Direction::Short;

        let (leverage_offer, leverage_accept) = match offer_party_direction {
            Direction::Long => (leverage_long, leverage_short),
            Direction::Short => (leverage_short, leverage_long),
        };

        let margin_offer =
            calculate_margin(initial_price, quantity, leverage_offer.to_f32().unwrap());
        let margin_accept =
            calculate_margin(initial_price, quantity, leverage_accept.to_f32().unwrap());

        let offer_party = PartyParams {
            margin: margin_offer,
            collateral_reserve: collateral_reserve_offer.to_sat(),
        };

        let accept_party = PartyParams {
            margin: margin_accept,
            collateral_reserve: collateral_reserve_accept.to_sat(),
        };

        let long_liquidation_price = calculate_long_bankruptcy_price(leverage_long, initial_price);
        let short_liquidation_price =
            calculate_short_bankruptcy_price(leverage_short, initial_price);
        let price_params = PriceParams {
            initial_price,
            long_liquidation_price,
            short_liquidation_price,
        };

        let payout_function = build_inverse_payout_function(
            quantity,
            offer_party,
            accept_party,
            price_params,
            offer_party_direction,
        )
        .unwrap();

        assert_debug_snapshot!(payout_function);
    }

    #[test]
    fn ensure_all_bounds_smaller_or_equal_max_btc_price() {
        // setup
        let quantity = 19.0;
        let initial_price = dec!(36780);
        let long_leverage = 2.0;
        let short_leverage = 1.0;

        let offer_margin =
            Amount::from_sat(calculate_margin(initial_price, quantity, long_leverage));
        let accept_margin =
            Amount::from_sat(calculate_margin(initial_price, quantity, short_leverage));

        let collateral_reserve_offer = Amount::from_sat(155);

        let long_liquidation_price = calculate_long_bankruptcy_price(
            Decimal::from_f32(long_leverage).expect("to fit into f32"),
            initial_price,
        );
        let short_liquidation_price = calculate_short_bankruptcy_price(
            Decimal::from_f32(short_leverage).expect("to fit into f32"),
            initial_price,
        );

        let party_params_offer = PartyParams::new(offer_margin, collateral_reserve_offer);
        let party_params_accept = PartyParams::new(accept_margin, Amount::ZERO);

        // act: offer long
        let offer_direction = Direction::Long;

        let mid_range_payouts_offer_long = calculate_mid_range_payouts(
            party_params_offer,
            party_params_accept,
            initial_price,
            &PayoutPoint {
                event_outcome: long_liquidation_price.to_u64().unwrap(),
                outcome_payout: party_params_offer.collateral_reserve,
                extra_precision: 0,
            },
            &PayoutPoint {
                event_outcome: short_liquidation_price.to_u64().unwrap(),
                outcome_payout: party_params_offer.total_collateral()
                    + party_params_accept.margin(),
                extra_precision: 0,
            },
            offer_direction,
            quantity,
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
    fn calculate_upper_range_payout_points_when_offer_short_then_gets_reserve() {
        // setup
        // we take 2 BTC so that all tests have nice numbers
        let total_collateral = Amount::ONE_BTC.to_sat() * 2;
        let liquidation_price_short = dec!(60_000);
        let collateral_reserve_offer = 300_000;

        // act
        let offer_direction = Direction::Short;

        let (lower, upper) = calculate_short_liquidation_interval_payouts(
            offer_direction,
            total_collateral,
            liquidation_price_short,
            collateral_reserve_offer,
        )
        .unwrap();

        // assert
        assert_eq!(lower.event_outcome, 60_000);
        assert_eq!(lower.outcome_payout, collateral_reserve_offer);
        assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
        assert_eq!(upper.outcome_payout, collateral_reserve_offer);

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
        let liquidation_price_short = dec!(60_000);
        let collateral_reserve_accept = 50_000;

        // act
        let offer_direction = Direction::Long;

        let (lower, upper) = calculate_short_liquidation_interval_payouts(
            offer_direction,
            total_collateral,
            liquidation_price_short,
            collateral_reserve_accept,
        )
        .unwrap();

        // assert
        assert_eq!(lower.event_outcome, 60_000);
        assert_eq!(
            lower.outcome_payout,
            total_collateral - collateral_reserve_accept
        );
        assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
        assert_eq!(
            upper.outcome_payout,
            total_collateral - collateral_reserve_accept
        );

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
        let collateral_reserve_accept = 300_000;

        // act
        let offer_direction = Direction::Long;

        let (lower, upper) = calculate_short_liquidation_interval_payouts(
            offer_direction,
            total_collateral,
            Decimal::from(BTCUSD_MAX_PRICE),
            collateral_reserve_accept,
        )
        .unwrap();

        // assert
        assert_eq!(lower.event_outcome, BTCUSD_MAX_PRICE - 1);
        assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
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
        collateral_reserve_offer: u64,
    }

    //******* Proptests *******//

    proptest! {
        #[test]
        fn calculating_lower_bound_doesnt_crash_offer_short(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let bound = Decimal::from_u64(bound).expect("to be able to parse bound");
            let collateral_reserve_long = total_collateral / 5;

            // act:
            let (lower_payout_lower, lower_payout_upper) =
                calculate_long_liquidation_interval_payouts(Direction::Short, total_collateral, bound, collateral_reserve_long).unwrap();

            // assert
            prop_assert_eq!(lower_payout_lower.event_outcome, 0);
            prop_assert_eq!(lower_payout_lower.outcome_payout, total_collateral - collateral_reserve_long);
            prop_assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
            prop_assert_eq!(lower_payout_upper.outcome_payout, total_collateral - collateral_reserve_long);
        }
    }

    proptest! {
        #[test]
        fn calculating_lower_bound_doesnt_crash_offer_long(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let bound = Decimal::from_u64(bound).expect("to be able to parse bound");
            let collateral_reserve_long = total_collateral / 5;

            // act:
            let (lower_payout_lower, lower_payout_upper) =
                calculate_long_liquidation_interval_payouts(Direction::Short, total_collateral, bound, collateral_reserve_long).unwrap();

            // assert
            prop_assert_eq!(lower_payout_lower.event_outcome, 0);
            prop_assert_eq!(lower_payout_lower.outcome_payout, total_collateral - collateral_reserve_long);
            prop_assert_eq!(lower_payout_upper.event_outcome, bound.to_u64().unwrap());
            prop_assert_eq!(lower_payout_upper.outcome_payout, total_collateral - collateral_reserve_long);
        }
    }

    proptest! {
        #[test]
        fn calculating_upper_bound_doesnt_crash_offer_short(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let collateral_reserve_short = total_collateral / 5;

            // act
            let offer_direction = Direction::Short;

            let (lower, upper) =
                calculate_short_liquidation_interval_payouts(offer_direction, total_collateral, Decimal::from(bound), collateral_reserve_short).unwrap();

            // assert
            prop_assert_eq!(lower.event_outcome, bound);
            prop_assert_eq!(lower.outcome_payout, collateral_reserve_short);
            prop_assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
            prop_assert_eq!(upper.outcome_payout, collateral_reserve_short);
        }

    }

    proptest! {
        #[test]
        fn calculating_upper_bound_doesnt_crash_offer_long(total_collateral in 1u64..100_000_000_000, bound in 1u64..100_000) {
            let collateral_reserve_short = total_collateral / 5;

            // act
            let offer_direction = Direction::Long;

            let (lower, upper) =
                calculate_short_liquidation_interval_payouts(offer_direction, total_collateral, Decimal::from(bound), collateral_reserve_short).unwrap();

            // assert
            assert_eq!(lower.event_outcome, bound);
            assert_eq!(lower.outcome_payout, total_collateral - collateral_reserve_short);
            assert_eq!(upper.event_outcome, BTCUSD_MAX_PRICE);
            assert_eq!(upper.outcome_payout, total_collateral - collateral_reserve_short);
        }
    }

    proptest! {
        #[test]
        fn midrange_always_positive(initial_price in 20_000i32..50_000, short_leverage in 1i32..5) {
            // setup
            let quantity = 1000.0;
            let initial_price = Decimal::from_i32(initial_price).expect("to be able to parse");
            let long_leverage = 2.0;
            let short_leverage = short_leverage as f32;

            let offer_margin =
                Amount::from_sat(calculate_margin(initial_price, quantity, long_leverage));
            let accept_margin =
                Amount::from_sat(calculate_margin(initial_price, quantity, short_leverage));

            // Collateral reserve for the offer party based on a fee calculation.
            let collateral_reserve_offer = {
                let collateral_reserve = dec!(0.003) * Decimal::from_f32(quantity).expect("to be able to parse into dec")
                    / initial_price;
                let collateral_reserve = collateral_reserve
                    .mul(dec!(100_000_000))
                    .to_u64()
                    .expect("to fit into u64");

                Amount::from_sat(collateral_reserve)
            };

            let long_liquidation_price = calculate_long_bankruptcy_price(
                Decimal::from_f32(long_leverage).expect("to fit into f32"),
                initial_price,
            );
            let short_liquidation_price = calculate_short_bankruptcy_price(
                Decimal::from_f32(short_leverage).expect("to fit into f32"),
                initial_price,
            );

            let party_params_offer = PartyParams::new(offer_margin, collateral_reserve_offer);
            let party_params_accept = PartyParams::new(accept_margin, Amount::ZERO);

            // act: offer long
            let offer_direction = Direction::Long;

            let mid_range_payouts_offer_long = calculate_mid_range_payouts(
                party_params_offer,
                party_params_accept,
                initial_price,
                &PayoutPoint {
                    event_outcome: long_liquidation_price.to_u64().unwrap(),
                    outcome_payout: party_params_offer.collateral_reserve,
                    extra_precision: 0,
                },
                &PayoutPoint {
                    event_outcome: short_liquidation_price.to_u64().unwrap(),
                    outcome_payout: party_params_offer.total_collateral() + party_params_accept.margin(),
                    extra_precision: 0,
                },
                offer_direction,
                quantity,
            )
            .expect("To be able to compute mid range");

            // assert
            mid_range_payouts_offer_long
                .iter()
                .all(|(lower, upper)| lower.outcome_payout > 0 && upper.outcome_payout > 0);
        }
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;
    use proptest::prelude::*;
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal_macros::dec;
    use trade::cfd::calculate_long_bankruptcy_price;
    use trade::cfd::calculate_margin;
    use trade::cfd::calculate_short_bankruptcy_price;

    #[test]
    fn correct_bounds_between_middle_and_liquidation_intervals() {
        use Direction::*;

        check(1.0, dec!(20_000), dec!(1), dec!(1), 0, 0, Short);
        check(500.0, dec!(28_251), dec!(2), dec!(2), 20_386, 15_076, Short);
    }

    proptest! {
        #[test]
        fn liquidation_intervals_always_sane_wrt_middle(
            quantity in 1.0f32..10_000.0,
            initial_price in 20_000u32..80_000,
            leverage_long in 1u32..5,
            leverage_short in 1u32..5,
            is_long in proptest::bool::ANY,
            collateral_reserve_short in 0u64..1_000_000,
            collateral_reserve_long in 0u64..1_000_000,
        ) {
            let initial_price = Decimal::from(initial_price);

            let offer_party_direction = if is_long {
                Direction::Long
            } else {
                Direction::Short
            };

            let (collateral_reserve_offer, collateral_reserve_accept) = match offer_party_direction {
                Direction::Long => (collateral_reserve_long, collateral_reserve_short),
                Direction::Short => (collateral_reserve_short, collateral_reserve_long),
            };

            check(
                quantity,
                initial_price,
                Decimal::from(leverage_long),
                Decimal::from(leverage_short),
                collateral_reserve_offer,
                collateral_reserve_accept,
                offer_party_direction
            );
        }
    }

    #[track_caller]
    fn check(
        quantity: f32,
        initial_price: Decimal,
        leverage_long: Decimal,
        leverage_short: Decimal,
        collateral_reserve_offer: u64,
        collateral_reserve_accept: u64,
        offer_party_direction: Direction,
    ) {
        let (leverage_offer, leverage_accept) = match offer_party_direction {
            Direction::Long => (leverage_long, leverage_short),
            Direction::Short => (leverage_short, leverage_long),
        };

        let margin_offer =
            calculate_margin(initial_price, quantity, leverage_offer.to_f32().unwrap());
        let margin_accept =
            calculate_margin(initial_price, quantity, leverage_accept.to_f32().unwrap());

        let offer_party = PartyParams {
            margin: margin_offer,
            collateral_reserve: collateral_reserve_offer,
        };

        let accept_party = PartyParams {
            margin: margin_accept,
            collateral_reserve: collateral_reserve_accept,
        };

        let long_liquidation_price = calculate_long_bankruptcy_price(leverage_long, initial_price);
        let short_liquidation_price =
            calculate_short_bankruptcy_price(leverage_short, initial_price);
        let price_params = PriceParams {
            initial_price,
            long_liquidation_price,
            short_liquidation_price,
        };

        let payout_function = build_inverse_payout_function(
            quantity,
            offer_party,
            accept_party,
            price_params,
            offer_party_direction,
        )
        .unwrap();

        let long_liquidation_payout_offer = match offer_party_direction {
            Direction::Long => offer_party.collateral_reserve,
            Direction::Short => offer_party.total_collateral() + accept_party.margin(),
        };

        let short_liquidation_payout_offer = match offer_party_direction {
            Direction::Long => offer_party.total_collateral() + accept_party.margin(),
            Direction::Short => offer_party.collateral_reserve,
        };

        assert_eq!(
            payout_function.first().unwrap().0.outcome_payout,
            long_liquidation_payout_offer
        );

        assert_eq!(
            payout_function.first().unwrap().1.outcome_payout,
            long_liquidation_payout_offer
        );

        assert_eq!(
            payout_function[1].0.outcome_payout,
            long_liquidation_payout_offer
        );

        assert_eq!(
            payout_function.last().unwrap().0.outcome_payout,
            short_liquidation_payout_offer
        );

        assert_eq!(
            payout_function.last().unwrap().1.outcome_payout,
            short_liquidation_payout_offer
        );

        assert_eq!(
            payout_function[payout_function.len() - 1].1.outcome_payout,
            short_liquidation_payout_offer
        );
    }
}
