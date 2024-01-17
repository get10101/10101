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
use payout_curve::ROUNDING_PERCENT;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use tracing::instrument;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_short_liquidation_price;
use trade::cfd::BTCUSD_MAX_PRICE;
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

    // The payout curve generation code tends to shift the liquidation prices slightly.
    let adjusted_long_liquidation_price = payout_points
        .first()
        .context("Empty payout points")?
        .1
        .event_outcome;
    let adjusted_short_liquidation_price = payout_points
        .last()
        .context("Empty payout points")?
        .0
        .event_outcome;

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

    let rounding_intervals = {
        let total_margin = coordinator_margin + trader_margin;

        create_rounding_intervals(
            total_margin,
            adjusted_long_liquidation_price,
            adjusted_short_liquidation_price,
        )
    };

    Ok((payout_function, rounding_intervals))
}

/// Returns the liquidation price for `(coordinator, maker)`
fn get_liquidation_prices(
    initial_price: Decimal,
    coordinator_direction: Direction,
    leverage_coordinator: Decimal,
    leverage_trader: Decimal,
) -> (Decimal, Decimal) {
    let (coordinator_liquidation_price, trader_liquidation_price) = match coordinator_direction {
        Direction::Long => (
            calculate_long_liquidation_price(leverage_coordinator, initial_price),
            calculate_short_liquidation_price(leverage_trader, initial_price),
        ),
        Direction::Short => (
            calculate_short_liquidation_price(leverage_coordinator, initial_price),
            calculate_long_liquidation_price(leverage_trader, initial_price),
        ),
    };
    (coordinator_liquidation_price, trader_liquidation_price)
}

pub fn create_rounding_intervals(
    total_margin: u64,
    long_liquidation_price: u64,
    short_liquidation_price: u64,
) -> RoundingIntervals {
    let liquidation_diff = short_liquidation_price
        .checked_sub(long_liquidation_price)
        .expect("short liquidation to be higher than long liquidation");
    let low_price = long_liquidation_price + liquidation_diff / 10;
    let high_price = short_liquidation_price - liquidation_diff / 10;

    let mut intervals = vec![
        RoundingInterval {
            begin_interval: 0,
            // No rounding.
            rounding_mod: 1,
        },
        // HACK: We decrease the rounding here to prevent `rust-dlc` from rounding under the long
        // liquidation price _payout_.
        RoundingInterval {
            begin_interval: long_liquidation_price,
            rounding_mod: (total_margin as f32 * ROUNDING_PERCENT * 0.1) as u64,
        },
        RoundingInterval {
            begin_interval: low_price,
            rounding_mod: (total_margin as f32 * ROUNDING_PERCENT) as u64,
        },
    ];

    if short_liquidation_price < BTCUSD_MAX_PRICE {
        intervals.push(
            // HACK: We decrease the rounding here to prevent `rust-dlc` from rounding over the
            // short liquidation price _payout_.
            RoundingInterval {
                begin_interval: high_price,
                rounding_mod: (total_margin as f32 * ROUNDING_PERCENT * 0.1) as u64,
            },
        );
        intervals.push(RoundingInterval {
            begin_interval: short_liquidation_price,
            // No rounding.
            rounding_mod: 1,
        })
    }

    RoundingIntervals { intervals }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commons::order_matching_fee_taker;
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

        let coordinator_collateral_reserve =
            order_matching_fee_taker(quantity, initial_price).to_sat();
        let trader_collateral_reserve = order_matching_fee_taker(quantity, initial_price).to_sat();

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
            ContractDescriptor::Numerical(numerical) => {
                numerical.get_range_payouts(total_collateral).unwrap()
            }
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
    fn build_contract_descriptor_dont_panic() {
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
