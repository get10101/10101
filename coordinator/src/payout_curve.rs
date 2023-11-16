use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use payout_curve::ROUNDING_PERCENT;
use rust_decimal::Decimal;
use tracing::instrument;
use trade::ContractSymbol;
use trade::Direction;

/// Builds the contract descriptor from the point of view of the coordinator.
///
/// It's the direction of the coordinator because the coordinator is always proposing
#[instrument]
#[allow(clippy::too_many_arguments)]
pub fn build_contract_descriptor(
    initial_price: Decimal,
    coordinator_margin: u64,
    trader_margin: u64,
    leverage_coordinator: f32,
    leverage_trader: f32,
    coordinator_direction: Direction,
    fee: u64,
    rounding_intervals: RoundingIntervals,
    quantity: f32,
    symbol: ContractSymbol,
) -> Result<ContractDescriptor> {
    if symbol != ContractSymbol::BtcUsd {
        bail!("We only support BTCUSD at the moment. For other symbols we will need a different payout curve");
    }

    tracing::info!("Building contract descriptor");

    Ok(ContractDescriptor::Numerical(NumericalDescriptor {
        payout_function: build_inverse_payout_function(
            coordinator_margin,
            trader_margin,
            initial_price,
            leverage_trader,
            leverage_coordinator,
            fee,
            coordinator_direction,
            quantity,
        )?,
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
#[allow(clippy::too_many_arguments)]
fn build_inverse_payout_function(
    coordinator_collateral: u64,
    trader_collateral: u64,
    initial_price: Decimal,
    leverage_trader: f32,
    leverage_coordinator: f32,
    fee: u64,
    coordinator_direction: Direction,
    quantity: f32,
) -> Result<PayoutFunction> {
    let payout_points = payout_curve::build_inverse_payout_function(
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

    Ok(payout_function)
}

pub fn create_rounding_interval(total_collateral: u64) -> RoundingIntervals {
    RoundingIntervals {
        intervals: vec![RoundingInterval {
            begin_interval: 0,
            rounding_mod: (total_collateral as f32 * ROUNDING_PERCENT) as u64,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderbook_commons::order_matching_fee_taker;
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

        let fee = order_matching_fee_taker(quantity, initial_price).to_sat();

        let total_collateral = coordinator_margin + trader_margin;
        let rounding_intervals = create_rounding_interval(total_collateral);

        let symbol = ContractSymbol::BtcUsd;

        let descriptor = build_contract_descriptor(
            initial_price,
            coordinator_margin,
            trader_margin,
            leverage_coordinator,
            leverage_trader,
            coordinator_direction,
            fee,
            rounding_intervals,
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
}
