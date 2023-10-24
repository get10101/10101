use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_short_liquidation_price;
use trade::cfd::BTCUSD_MAX_PRICE;

/// Builds a [`PayoutFunction`].
///
/// TODO: We are currently building a linear payout function for
/// simplicity. This is *wrong*. We should build an inverse payout
/// function like we used to do in ItchySats.
pub fn build_payout_function(
    total_collateral: u64,
    initial_price: Decimal,
    leverage_long: f32,
    leverage_short: f32,
    fee: u64,
) -> anyhow::Result<PayoutFunction> {
    let leverage_short = Decimal::try_from(leverage_short)?;
    let liquidation_price_short = calculate_short_liquidation_price(leverage_short, initial_price);

    let leverage_long = Decimal::try_from(leverage_long)?;
    let liquidation_price_long = calculate_long_liquidation_price(leverage_long, initial_price);

    let lower_limit = liquidation_price_long
        .floor()
        .to_u64()
        .expect("Failed to fit floored liquidation price to u64");
    let upper_limit = liquidation_price_short
        .floor()
        .to_u64()
        .expect("Failed to fit floored liquidation price to u64");

    let lower_range = PolynomialPayoutCurvePiece::new(vec![
        PayoutPoint {
            event_outcome: 0,
            outcome_payout: fee,
            extra_precision: 0,
        },
        PayoutPoint {
            event_outcome: lower_limit,
            outcome_payout: fee,
            extra_precision: 0,
        },
    ])?;

    let middle_range = PolynomialPayoutCurvePiece::new(vec![
        PayoutPoint {
            event_outcome: lower_limit,
            outcome_payout: fee,
            extra_precision: 0,
        },
        PayoutPoint {
            event_outcome: upper_limit,
            outcome_payout: total_collateral,
            extra_precision: 0,
        },
    ])?;

    let mut pieces = vec![
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(lower_range),
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(middle_range),
    ];

    // When the upper limit is greater than or equal to the
    // `BTCUSD_MAX_PRICE`, we don't have to add another curve piece.
    if upper_limit < BTCUSD_MAX_PRICE {
        let upper_range = PolynomialPayoutCurvePiece::new(vec![
            PayoutPoint {
                event_outcome: upper_limit,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
        ])?;

        pieces.push(PayoutFunctionPiece::PolynomialPayoutCurvePiece(upper_range));
    }

    let payout_function = PayoutFunction::new(pieces)?;

    Ok(payout_function)
}
