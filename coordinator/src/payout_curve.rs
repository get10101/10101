use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use tracing::instrument;
use trade::cfd::calculate_long_bankruptcy_price;
use trade::cfd::calculate_short_bankruptcy_price;
use trade::ContractSymbol;
use trade::Direction;

/// Builds the contract descriptor from the point of view of the coordinator.
///
/// It's the direction of the coordinator because the coordinator is always proposing.
#[instrument]
#[allow(clippy::too_many_arguments)]
pub fn build_contract_descriptor(
    initial_price: Decimal,
    coordinator_margin: u64,
    trader_margin: u64,
    leverage_coordinator: f32,
    leverage_trader: f32,
    coordinator_direction: Direction,
    coordinator_collateral_reserve: u64,
    trader_collateral_reserve: u64,
    quantity: f32,
    symbol: ContractSymbol,
) -> Result<ContractDescriptor> {
    ensure!(
        symbol == ContractSymbol::BtcUsd,
        "We only support BTCUSD at the moment. \
         For other symbols we will need a different payout curve"
    );

    tracing::info!("Building contract descriptor");

    let (payout_function, rounding_intervals) = build_inverse_payout_function(
        coordinator_margin,
        trader_margin,
        initial_price,
        leverage_trader,
        leverage_coordinator,
        coordinator_collateral_reserve,
        trader_collateral_reserve,
        coordinator_direction,
        quantity,
    )?;

    Ok(ContractDescriptor::Numerical(NumericalDescriptor {
        payout_function,
        rounding_intervals,
        difference_params: None,
        oracle_numeric_infos: dlc_trie::OracleNumericInfo {
            base: 2,
            nb_digits: vec![20],
        },
    }))
}

/// Build a [`PayoutFunction`] for an inverse perpetual future e.g. BTCUSD. Perspective is always
/// from the person who offers, i.e. in our case from the coordinator.
///
/// Additionally returns the [`RoundingIntervals`] to indicate how it should be discretized.
#[allow(clippy::too_many_arguments)]
fn build_inverse_payout_function(
    // TODO: The `coordinator_margin` and `trader_margin` are _not_ orthogonal to the other
    // arguments passed in.
    coordinator_margin: u64,
    trader_margin: u64,
    initial_price: Decimal,
    leverage_trader: f32,
    leverage_coordinator: f32,
    coordinator_collateral_reserve: u64,
    trader_collateral_reserve: u64,
    coordinator_direction: Direction,
    quantity: f32,
) -> Result<(PayoutFunction, RoundingIntervals)> {
    let leverage_coordinator =
        Decimal::from_f32(leverage_coordinator).expect("to fit into decimal");
    let leverage_trader = Decimal::from_f32(leverage_trader).expect("to fit into decimal");

    let (coordinator_liquidation_price, trader_liquidation_price) = get_liquidation_prices(
        initial_price,
        coordinator_direction,
        leverage_coordinator,
        leverage_trader,
    );

    let (long_liquidation_price, short_liquidation_price) = match coordinator_direction {
        Direction::Long => (coordinator_liquidation_price, trader_liquidation_price),
        Direction::Short => (trader_liquidation_price, coordinator_liquidation_price),
    };

    let price_params = payout_curve::PriceParams::new_btc_usd(
        initial_price,
        long_liquidation_price,
        short_liquidation_price,
    )?;

    let party_params_coordinator = payout_curve::PartyParams::new(
        Amount::from_sat(coordinator_margin),
        Amount::from_sat(coordinator_collateral_reserve),
    );
    let party_params_trader = payout_curve::PartyParams::new(
        Amount::from_sat(trader_margin),
        Amount::from_sat(trader_collateral_reserve),
    );

    let payout_points = payout_curve::build_inverse_payout_function(
        quantity,
        party_params_coordinator,
        party_params_trader,
        price_params,
        coordinator_direction,
    )?;

    let mut pieces = vec![];
    for (lower, upper) in payout_points {
        let lower_range = PolynomialPayoutCurvePiece::new(vec![
            PayoutPoint {
                event_outcome: lower.event_outcome,
                outcome_payout: lower.outcome_payout,
                extra_precision: lower.extra_precision,
            },
            PayoutPoint {
                event_outcome: upper.event_outcome,
                outcome_payout: upper.outcome_payout,
                extra_precision: upper.extra_precision,
            },
        ])?;
        pieces.push(PayoutFunctionPiece::PolynomialPayoutCurvePiece(lower_range));
    }

    let payout_function =
        PayoutFunction::new(pieces).context("could not create payout function")?;

    let rounding_intervals = RoundingIntervals {
        intervals: vec![RoundingInterval {
            begin_interval: 0,
            // No rounding needed because we are giving `rust-dlc` a step function already.
            rounding_mod: 1,
        }],
    };

    Ok((payout_function, rounding_intervals))
}

/// Returns the liquidation price for `(coordinator, maker)` with a maintenance margin of 0%. also
/// known as the bankruptcy price.
fn get_liquidation_prices(
    initial_price: Decimal,
    coordinator_direction: Direction,
    leverage_coordinator: Decimal,
    leverage_trader: Decimal,
) -> (Decimal, Decimal) {
    let (coordinator_liquidation_price, trader_liquidation_price) = match coordinator_direction {
        Direction::Long => (
            calculate_long_bankruptcy_price(leverage_coordinator, initial_price),
            calculate_short_bankruptcy_price(leverage_trader, initial_price),
        ),
        Direction::Short => (
            calculate_short_bankruptcy_price(leverage_coordinator, initial_price),
            calculate_long_bankruptcy_price(leverage_trader, initial_price),
        ),
    };
    (coordinator_liquidation_price, trader_liquidation_price)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rust_decimal_macros::dec;
    use trade::cfd::calculate_margin;

    #[test]
    fn payout_price_range_is_below_max_price() {
        let initial_price = dec!(36780);
        let quantity = 19.0;
        let leverage_coordinator = 2.0;
        let coordinator_margin = calculate_margin(initial_price, quantity, leverage_coordinator);

        let leverage_trader = 1.0;
        let trader_margin = calculate_margin(initial_price, quantity, leverage_trader);

        let coordinator_direction = Direction::Long;

        let coordinator_collateral_reserve = Amount::from_sat(1000).to_sat();
        let trader_collateral_reserve = Amount::from_sat(1000).to_sat();

        let total_collateral = coordinator_margin + trader_margin;

        let symbol = ContractSymbol::BtcUsd;

        let descriptor = build_contract_descriptor(
            initial_price,
            coordinator_margin,
            trader_margin,
            leverage_coordinator,
            leverage_trader,
            coordinator_direction,
            coordinator_collateral_reserve,
            trader_collateral_reserve,
            quantity,
            symbol,
        )
        .unwrap();

        let range_payouts = match descriptor {
            ContractDescriptor::Enum(_) => unreachable!(),
            ContractDescriptor::Numerical(numerical) => numerical
                .get_range_payouts(
                    total_collateral + coordinator_collateral_reserve + trader_collateral_reserve,
                )
                .unwrap(),
        };

        let max_price = 2usize.pow(20);

        for range_payout in &range_payouts {
            assert!(
                range_payout.start + range_payout.count <= max_price,
                "{} + {} = {} > {}",
                range_payout.start,
                range_payout.count,
                range_payout.start + range_payout.count,
                max_price
            );
        }
    }

    #[test]
    /// We check that the generated payout function takes into account the provided collateral
    /// reserves. A party's collateral reserve is their coins in the DLC channel that are not being
    /// wagered. As such, we expect _any_ of their payouts to be _at least_ their collateral
    /// reserve.
    fn payout_function_respects_collateral_reserve() {
        // Arrange

        let initial_price = dec!(28_251);
        let quantity = 500.0;
        let leverage_offer = 2.0;
        let margin_offer = calculate_margin(initial_price, quantity, leverage_offer);

        let leverage_accept = 2.0;
        let margin_accept = calculate_margin(initial_price, quantity, leverage_accept);

        let direction_offer = Direction::Short;

        let collateral_reserve_offer = 2_120_386;
        let collateral_reserve_accept = 5_115_076;

        let total_collateral =
            margin_offer + margin_accept + collateral_reserve_offer + collateral_reserve_accept;

        let symbol = ContractSymbol::BtcUsd;

        // Act

        let descriptor = build_contract_descriptor(
            initial_price,
            margin_offer,
            margin_accept,
            leverage_offer,
            leverage_accept,
            direction_offer,
            collateral_reserve_offer,
            collateral_reserve_accept,
            quantity,
            symbol,
        )
        .unwrap();

        // Assert

        // Extract the payouts from the generated `ContractDescriptor`.
        let range_payouts = match descriptor {
            ContractDescriptor::Enum(_) => unreachable!(),
            ContractDescriptor::Numerical(numerical) => {
                numerical.get_range_payouts(total_collateral).unwrap()
            }
        };

        // The offer party gets liquidated when they get the minimum amount of sats as a payout.
        let liquidation_payout_offer = range_payouts
            .iter()
            .min_by(|a, b| a.payout.offer.cmp(&b.payout.offer))
            .unwrap()
            .payout
            .offer;

        // The minimum amount the offer party can get as a payout is their collateral reserve.
        assert_eq!(liquidation_payout_offer, collateral_reserve_offer);

        // The accept party gets liquidated when they get the minimum amount of sats as a payout.
        let liquidation_payout_accept = range_payouts
            .iter()
            .min_by(|a, b| a.payout.accept.cmp(&b.payout.accept))
            .unwrap()
            .payout
            .accept;

        // The minimum amount the accept party can get as a payout is their collateral reserve.
        assert_eq!(liquidation_payout_accept, collateral_reserve_accept);
    }

    proptest! {
        #[test]
        fn payout_function_always_respects_reserves(
            quantity in 1.0f32..10_000.0,
            initial_price in 20_000u32..80_000,
            leverage_coordinator in 1u32..5,
            leverage_trader in 1u32..5,
            is_coordinator_long in proptest::bool::ANY,
            collateral_reserve_coordinator in 0u64..1_000_000,
            collateral_reserve_trader in 0u64..1_000_000,
        ) {
            let initial_price = Decimal::from(initial_price);
            let leverage_coordinator = leverage_coordinator as f32;
            let leverage_trader = leverage_trader as f32;

            let margin_coordinator = calculate_margin(initial_price, quantity, leverage_coordinator);
            let margin_trader = calculate_margin(initial_price, quantity, leverage_trader);

            let coordinator_direction = if is_coordinator_long {
                Direction::Long
            } else {
                Direction::Short
            };

            let total_collateral = margin_coordinator
                + margin_trader
                + collateral_reserve_coordinator
                + collateral_reserve_trader;

            let symbol = ContractSymbol::BtcUsd;

            let descriptor = build_contract_descriptor(
                initial_price,
                margin_coordinator,
                margin_trader,
                leverage_coordinator,
                leverage_trader,
                coordinator_direction,
                collateral_reserve_coordinator,
                collateral_reserve_trader,
                quantity,
                symbol,
            )
                .unwrap();

            let range_payouts = match descriptor {
                ContractDescriptor::Enum(_) => unreachable!(),
                ContractDescriptor::Numerical(numerical) => numerical
                    .get_range_payouts(total_collateral)
                    .unwrap(),
            };

            let liquidation_payout_offer = range_payouts
                .iter()
                .min_by(|a, b| a.payout.offer.cmp(&b.payout.offer))
                .unwrap()
                .payout
                .offer;

            assert_eq!(liquidation_payout_offer, collateral_reserve_coordinator);

            let liquidation_payout_accept = range_payouts
                .iter()
                .min_by(|a, b| a.payout.accept.cmp(&b.payout.accept))
                .unwrap()
                .payout
                .accept;

            assert_eq!(liquidation_payout_accept, collateral_reserve_trader);
        }
    }

    #[test]
    fn calculate_liquidation_price_coordinator_long() {
        let initial_price = dec!(30_000);
        let coordinator_direction = Direction::Long;
        let leverage_coordinator = dec!(2.0);
        let leverage_trader = dec!(3.0);

        let (coordinator, maker) = get_liquidation_prices(
            initial_price,
            coordinator_direction,
            leverage_coordinator,
            leverage_trader,
        );

        assert_eq!(coordinator, dec!(20_000));
        assert_eq!(maker, dec!(45_000));
    }

    #[test]
    fn calculate_liquidation_price_coordinator_short() {
        let initial_price = dec!(30_000);
        let coordinator_direction = Direction::Short;
        let leverage_coordinator = dec!(2.0);
        let leverage_trader = dec!(3.0);

        let (coordinator, maker) = get_liquidation_prices(
            initial_price,
            coordinator_direction,
            leverage_coordinator,
            leverage_trader,
        );

        assert_eq!(coordinator, dec!(60_000));
        assert_eq!(maker, dec!(22_500));
    }

    #[test]
    fn build_contract_descriptor_does_not_panic() {
        let initial_price = dec!(36404.5);
        let quantity = 20.0;
        let leverage_coordinator = 2.0;
        let coordinator_margin = 18_313;

        let leverage_trader = 3.0;
        let trader_margin = 27_469;

        let coordinator_direction = Direction::Short;

        let coordinator_collateral_reserve = 0;
        let trader_collateral_reserve = 0;

        let symbol = ContractSymbol::BtcUsd;

        let _descriptor = build_contract_descriptor(
            initial_price,
            coordinator_margin,
            trader_margin,
            leverage_coordinator,
            leverage_trader,
            coordinator_direction,
            coordinator_collateral_reserve,
            trader_collateral_reserve,
            quantity,
            symbol,
        )
        .unwrap();
    }
}
