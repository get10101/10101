use crate::calculations;
use crate::calculations::calculate_pnl;
use crate::channel_trade_constraints::channel_trade_constraints;
use crate::ln_dlc;
use crate::trade::position;
use bitcoin::Amount;
use bitcoin::SignedAmount;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::cmp::max;
use trade::Direction;
use trade::Price;

/// Calculates the max quantity a user can trade using the following input parameters
/// - if no channel exists the on-chain fees (channel fee reserve and funding tx fee) is substracted
///   from the max balance. Note, we add a little bit of buffer since these values are only
///   estimates.
/// - The max coordinator margin which is restricted to a certain max amount.
/// - The max trader margin which is either the on-chain balance or the off-chain balance if a
///   channel already exists.
pub fn max_quantity(
    price: Decimal,
    trader_leverage: f32,
    trader_trade_direction: Direction,
) -> anyhow::Result<Decimal> {
    let channel_trade_constraints = channel_trade_constraints()?;

    let on_chain_fee_estimate = match channel_trade_constraints.is_channel_balance {
        true => None,
        false => {
            let channel_fee_reserve = ln_dlc::estimated_fee_reserve()?;
            let funding_tx_fee = ln_dlc::estimated_funding_tx_fee()?;
            // double the funding tx fee to ensure we have enough buffer
            let funding_tx_with_buffer = funding_tx_fee * 2;

            Some(channel_fee_reserve + funding_tx_with_buffer)
        }
    };

    let max_coordinator_balance =
        Amount::from_sat(channel_trade_constraints.max_counterparty_balance_sats);
    let max_trader_balance = Amount::from_sat(channel_trade_constraints.max_local_balance_sats);
    let order_matching_fee_rate = channel_trade_constraints.order_matching_fee_rate;
    let order_matching_fee_rate = Decimal::try_from(order_matching_fee_rate).expect("to fit");

    let positions = position::handler::get_positions()?;
    let position = positions.first();
    let accumulated_order_matching_fees = position
        .map(|p| p.order_matching_fees)
        .unwrap_or(Amount::ZERO);

    let (max_coordinator_margin, max_trader_margin) = match position {
        Some(position) if trader_trade_direction != position.direction => {
            let total_collateral = channel_trade_constraints.total_collateral.unwrap_or(0);

            let total_balance = channel_trade_constraints.max_counterparty_balance_sats
                + channel_trade_constraints.max_local_balance_sats;
            let total_balance = Amount::from_sat(total_balance);

            let trader_margin = Amount::from_sat(position.collateral);
            let coordinator_margin = Amount::from_sat(total_collateral)
                .checked_sub(total_balance + trader_margin)
                .unwrap_or(Amount::ZERO);

            let trader_pnl = calculate_pnl(
                position.average_entry_price,
                Price {
                    ask: price,
                    bid: price,
                },
                position.quantity,
                position.leverage,
                position.direction,
            )?;

            let max_coordinator_margin =
                max_coordinator_balance.to_signed()? + coordinator_margin.to_signed()?;
            let max_trader_margin = max_trader_balance.to_signed()? + trader_margin.to_signed()?;
            let trader_pnl = SignedAmount::from_sat(trader_pnl);

            let max_coordinator_margin = max_coordinator_margin
                .checked_sub(trader_pnl)
                .unwrap_or(SignedAmount::ZERO);
            let max_trader_margin = max_trader_margin
                .checked_add(trader_pnl)
                .unwrap_or(SignedAmount::ZERO);

            debug_assert!(
                max_coordinator_margin.is_positive(),
                "max coordinator margin must be positive after subtracting the trader pnl"
            );
            debug_assert!(
                max_trader_margin.is_positive(),
                "max trader margin must be positive after adding the trader pnl"
            );

            (
                Amount::from_sat(max(0, max_coordinator_margin.to_sat()) as u64),
                Amount::from_sat(max(0, max_trader_margin.to_sat()) as u64),
            )
        }

        _ => (max_coordinator_balance, max_trader_balance),
    };

    let open_quantity = match position {
        Some(position) if position.direction != trader_trade_direction => {
            Decimal::try_from(position.quantity).expect("to fit")
        }
        _ => Decimal::ZERO,
    };

    let max_quantity = calculate_max_quantity(
        price,
        max_coordinator_margin,
        max_trader_margin,
        on_chain_fee_estimate,
        channel_trade_constraints.coordinator_leverage,
        trader_leverage,
        order_matching_fee_rate,
        accumulated_order_matching_fees,
        open_quantity,
    );

    Ok(max_quantity)
}

/// Calculates the max quantity for the given input parameters. If an on-chai fee estimate is
/// provided the max margins are reduced by that amount to ensure the fees are considered.
///
/// 1. Calculate the max coordinator quantity and max trader quantity.
/// 2. The smaller quantity is used to derive the order matching fee.
/// 3. Reduce the max margin by the order matching fee.
/// 4. Recalculate and return the max quantity from the reduced margin.
///
/// Note, this function will not exactly find the max quantity possible, but a very close
/// approximation.
#[allow(clippy::too_many_arguments)]
fn calculate_max_quantity(
    price: Decimal,
    max_coordinator_margin: Amount,
    max_trader_margin: Amount,
    on_chain_fee_estimate: Option<Amount>,
    coordinator_leverage: f32,
    trader_leverage: f32,
    order_matching_fee_rate: Decimal,
    accumulated_order_matching_fees: Amount,
    open_quantity: Decimal,
) -> Decimal {
    // subtract required on-chain fees with buffer if the trade is opening a channel.
    let max_coordinator_margin = max_coordinator_margin
        .checked_sub(on_chain_fee_estimate.unwrap_or(Amount::ZERO))
        .unwrap_or(Amount::ZERO)
        .checked_sub(accumulated_order_matching_fees)
        .unwrap_or(Amount::ZERO);
    let max_trader_margin = max_trader_margin
        .checked_sub(on_chain_fee_estimate.unwrap_or(Amount::ZERO))
        .unwrap_or(Amount::ZERO);

    let price_f32 = price.to_f32().expect("to fit");

    let max_trader_quantity =
        calculations::calculate_quantity(price_f32, max_trader_margin.to_sat(), trader_leverage);
    let max_coordinator_quantity = calculations::calculate_quantity(
        price_f32,
        max_coordinator_margin.to_sat(),
        coordinator_leverage,
    );

    // determine the biggest quantity possible from either side.
    let (quantity, max_margin, leverage) = match max_trader_quantity > max_coordinator_quantity {
        true => (
            max_coordinator_quantity,
            max_coordinator_margin,
            coordinator_leverage,
        ),
        false => (max_trader_quantity, max_trader_margin, trader_leverage),
    };

    // calculate the fee from this quantity + any open quantity to ensure there is enough space for
    // the fees.
    let open_quantity = open_quantity.to_f32().expect("to fit");
    let order_matching_fee =
        commons::order_matching_fee(quantity + open_quantity, price, order_matching_fee_rate);

    // subtract the fee from the max margin and recalculate the quantity. That
    // might not be perfect but the closest we can get with a relatively simple logic.
    let max_margin_without_order_matching_fees = max_margin - order_matching_fee;

    let max_quantity = calculations::calculate_quantity(
        price_f32,
        max_margin_without_order_matching_fees.to_sat(),
        leverage,
    );

    Decimal::try_from((max_quantity + open_quantity).floor()).expect("to fit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_max_quantity_with_open_quantity() {
        let price = Decimal::new(22001, 0);

        let max_coordinator_margin = Amount::from_sat(765_763);
        let max_trader_margin = Amount::from_sat(747_499);

        let trader_leverage = 2.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);
        let open_quantity = dec!(323);
        let accumulated_order_matching_fee = Amount::from_sat(4459);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            None,
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            accumulated_order_matching_fee,
            open_quantity,
        );

        // max trader quantity: 0.00,747,499 * 22,001 * 2.0 = 328,91450998
        // order matching fee: (328,91450998 + 323) * (1/22,001) * 0.003 = 0.00,008,889 BTC
        // max trader margin without order matching fee: 747,499 - 8,889 = 738,610
        // max quantity without order matching fee: 0.00,738,610 * 22,001 * 2.0 = 325,0031722
        // 325 + 323 = 648
        assert_eq!(dec!(648), max_quantity);

        // Ensure that the coordinator has enough funds for the trade
        let coordinator_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            (max_quantity - open_quantity).to_f32().unwrap(),
            coordinator_leverage,
        );

        assert!(coordinator_margin < max_coordinator_margin.to_sat());
    }

    #[test]
    fn test_calculate_max_quantity_with_accumulated_order_matching_fee() {
        let price = Decimal::new(14999, 0);

        let max_coordinator_margin = Amount::from_sat(7464);
        let max_trader_margin = Amount::from_sat(1_048_951);

        let trader_leverage = 2.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            None,
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            Amount::from_sat(4500),
            dec!(0),
        );

        assert_eq!(Decimal::ZERO, max_quantity);
    }

    #[test]
    fn test_calculate_max_quantity() {
        let price = Decimal::new(30209, 0);

        let max_coordinator_margin = Amount::from_sat(3_000_000);
        let max_trader_margin = Amount::from_sat(280_000);

        let on_chain_fee_estimate = Amount::from_sat(13_500);

        let trader_leverage = 2.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            Some(on_chain_fee_estimate),
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            Amount::ZERO,
            dec!(0),
        );

        let trader_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            trader_leverage,
        );

        let order_matching_fee = commons::order_matching_fee(
            max_quantity.to_f32().unwrap(),
            price,
            order_matching_fee_rate,
        );

        // Note this is not exactly the max margin the trader, but its the closest we can get.
        assert_eq!(
            Amount::from_sat(trader_margin) + on_chain_fee_estimate + order_matching_fee,
            // max trader margin: 280,000 - 13.500 = 266,500
            // max trader quantity: 0.00,266,500 * 30,209 * 2.0 = 161,01397
            // order matching fee: 161,01397 * (1/30,209) * 0.003 = 0.00,001,599 BTC
            // max trader margin without order matching fee: 266,500 - 1,599 = 264,901
            // max quantity without order matching fee: 0.00,264,901 * 30,209 * 2.0 = 160,04788618

            // trader margin: 160 / (30,209 * 2.0) = 0.00,264,821 BTC
            // order matching fee: 160 * (1/30,209) * 0,003 = 0.00,001,589 BTC
            // 264,822 + 13,500 + 1589
            Amount::from_sat(279_911)
        );

        // Ensure that the trader still has enough for the order matching fee
        assert!(Amount::from_sat(trader_margin) + order_matching_fee < max_trader_margin,
                "Trader does not have enough margin left for order matching fee. Has {}, order matching fee {}, needed for order {} ",
                max_trader_margin, order_matching_fee , trader_margin);

        // Ensure that the coordinator has enough funds for the trade
        let coordinator_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            coordinator_leverage,
        );
        assert!(coordinator_margin < max_coordinator_margin.to_sat());
    }

    #[test]
    fn test_calculate_max_quantity_with_smaller_coordinator_margin() {
        let price = Decimal::new(30209, 0);

        let max_coordinator_margin = Amount::from_sat(280_000);
        let max_trader_margin = Amount::from_sat(280_001);

        let trader_leverage = 2.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            None,
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            Amount::ZERO,
            dec!(0),
        );

        let trader_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            trader_leverage,
        );

        let order_matching_fee = commons::order_matching_fee(
            max_quantity.to_f32().unwrap(),
            price,
            order_matching_fee_rate,
        );

        // Note this is not exactly the max margin of the coordinator, but its the closest we can
        // get.
        assert_eq!(trader_margin, 278_063);

        // Ensure that the trader still has enough for the order matching fee
        assert!(Amount::from_sat(trader_margin) + order_matching_fee < max_trader_margin,
                "Trader does not have enough margin left for order matching fee. Has {}, order matching fee {}, needed for order {} ",
                max_trader_margin, order_matching_fee , trader_margin);

        // Ensure that the coordinator has enough funds for the trade
        let coordinator_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            coordinator_leverage,
        );
        assert!(
            Amount::from_sat(coordinator_margin) < max_coordinator_margin,
            "Coordinator does not have enough margin for the trade. Has {}, needed for order {} ",
            max_coordinator_margin,
            coordinator_margin
        );
    }

    #[test]
    fn test_calculate_max_quantity_with_higher_trader_leverage() {
        let price = Decimal::new(30209, 0);

        let max_coordinator_margin = Amount::from_sat(450_000);
        let max_trader_margin = Amount::from_sat(280_000);

        let trader_leverage = 5.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            None,
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            Amount::ZERO,
            dec!(0),
        );

        let trader_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            trader_leverage,
        );

        let order_matching_fee = commons::order_matching_fee(
            max_quantity.to_f32().unwrap(),
            price,
            order_matching_fee_rate,
        );

        // Note we can not max out the users balance, because the counterparty does not have enough
        // funds to match that trade on a leverage 2.0
        assert_eq!(Amount::from_sat(trader_margin), Amount::from_sat(178_755));

        // Ensure that the trader still has enough for the order matching fee
        assert!(Amount::from_sat(trader_margin) + order_matching_fee < max_trader_margin,
                "Trader does not have enough margin left for order matching fee. Has {}, order matching fee {}, needed for order {} ",
                max_trader_margin, order_matching_fee , trader_margin);

        // Ensure that the coordinator has enough funds for the trade
        let coordinator_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            coordinator_leverage,
        );

        // Note this is not the max coordinator balance, but the closest we can get.
        assert_eq!(coordinator_margin, 446_887);
    }

    #[test]
    fn test_calculate_max_quantity_zero_balance() {
        let price = Decimal::from(30353);

        let max_coordinator_margin = Amount::from_sat(3_000_000);
        let max_trader_margin = Amount::from_sat(0);

        let trader_leverage = 2.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);

        let on_chain_fee_estimate = Amount::from_sat(1515);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            Some(on_chain_fee_estimate),
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            Amount::ZERO,
            dec!(0),
        );

        assert_eq!(max_quantity, Decimal::ZERO)
    }

    #[test]
    fn test_calculate_max_quantity_with_max_channel_size() {
        let price = Decimal::new(28409, 0);

        let max_coordinator_margin = Amount::from_sat(3_000_000);
        let max_trader_margin = Amount::from_btc(1.0).unwrap();

        let trader_leverage = 2.0;
        let coordinator_leverage = 2.0;
        let order_matching_fee_rate = dec!(0.003);

        let on_chain_fee_estimate = Amount::from_sat(1515);

        let max_quantity = calculate_max_quantity(
            price,
            max_coordinator_margin,
            max_trader_margin,
            Some(on_chain_fee_estimate),
            coordinator_leverage,
            trader_leverage,
            order_matching_fee_rate,
            Amount::ZERO,
            dec!(0),
        );

        let trader_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            trader_leverage,
        );

        let order_matching_fee = commons::order_matching_fee(
            max_quantity.to_f32().unwrap(),
            price,
            order_matching_fee_rate,
        );

        // Note we can not max out the users balance, because the counterparty does not have enough
        // funds to match that trade on a leverage 2.0
        assert_eq!(trader_margin, 2_979_690);

        // Ensure that the trader still has enough for the order matching fee
        assert!(Amount::from_sat(trader_margin) + order_matching_fee < max_trader_margin,
                "Trader does not have enough margin left for order matching fee. Has {}, order matching fee {}, needed for order {} ",
                max_trader_margin, order_matching_fee , trader_margin);

        // Ensure that the coordinator has enough funds for the trade
        let coordinator_margin = calculations::calculate_margin(
            price.to_f32().unwrap(),
            max_quantity.to_f32().unwrap(),
            coordinator_leverage,
        );

        // Note this is not the max coordinator balance, but the closest we can get.
        assert!(
            Amount::from_sat(coordinator_margin) < max_coordinator_margin,
            "Coordinator does not have enough margin for the trade. Has {}, needed for order {} ",
            max_coordinator_margin,
            coordinator_margin
        );
    }
}
