use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use lightning::ln::channelmanager::ChannelDetails;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;
use trade::TradeParams;

// todo: in case of leverage 1.0 the liquidation will be set to 21000000. I guess this will result
// in issues when the overall upper boundary is smaller than that. It looks like the payout function
// will never finish in that case. Setting this value temporarily to 2.0 so that the liquidation
// price is below that total upper boundary.
const COORDINATOR_LEVERAGE: f64 = 2.0;

pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
}

impl Node {
    pub async fn trade(&self, trade_params: &TradeParams) -> Result<()> {
        // todo: eventually we will have to cover the following scenarios here.
        // 1. open a completely new position (no pre-existing position)
        // 2. extending a position, meaning that the position will be updated with a bigger
        // quantity.
        // 3. reducing a position, meaning that the position will be updated with
        // a smaller quantity, and the trader will get a payout.
        // 4. closing a position, if a the trade reversed the existing position.

        if self.is_position_closing(trade_params)? {
            self.close_position(trade_params).await
        } else {
            self.open_position(trade_params).await
        }
    }

    /// Checks if the existing position needs to be closed.
    fn is_position_closing(&self, trade_params: &TradeParams) -> Result<bool> {
        // FIXME: this needs to be replaced with a query from the database, matching the existing
        // position on quantity and direction. If a "reversed" order was found we propose to
        // collaboratively settle the dlc channel. If not the position either needs to be extended
        // or reduced. e.g. if we have an existing long position of a quantity of 100 and we get a
        // short position of a quantity of 75, we will simply reduce the existing dlc_channel to 25.
        // For MVP we do not support partially closing or extending a position, hence we can assume
        // that if a dlc has been found that means we need to close the position.

        // TODO: once we store the positions on the coordinator we can at least return a validation
        // error if the trader attempts to extend or partially close a position.

        Ok(self
            .inner
            .get_sub_channel_signed(&trade_params.pubkey)?
            .iter()
            .len()
            > 0)
    }

    async fn close_position(&self, trade_params: &TradeParams) -> Result<()> {
        tracing::info!("Closing position");

        tracing::info!("get margins");
        let (margin_coordinator, margin_trader) = get_margins(trade_params);
        tracing::info!("total collatoral");
        let total_collateral = margin_coordinator + margin_trader;
        tracing::info!("get leverages");
        let (leverage_long, leverage_short) = get_leverages(trade_params);

        tracing::info!("build payout curve");
        let payout_function = build_payout_curve(
            total_collateral,
            trade_params.execution_price,
            leverage_long,
            leverage_short,
        )?;

        tracing::info!("starting payout function");

        let accept_settlement_amount = payout_function
            .to_range_payouts(total_collateral, &get_rounding_intervals())
            .map_err(|e| anyhow!("{e:#}"))?
            .iter()
            .find(|p| trade_params.execution_price < (p.start + p.count) as f64)
            .map(|p| p.payout.accept)
            .context("Failed to find payout.")?;

        tracing::info!("finished payout function");

        tracing::debug!(
            "Settling position of {accept_settlement_amount} with {}",
            trade_params.pubkey
        );

        let channel_details = self.get_counterparty_channel(trade_params.pubkey)?;
        self.inner.propose_dlc_channel_collaborative_settlement(
            &channel_details.channel_id,
            accept_settlement_amount,
        )?;

        Ok(())
    }

    async fn open_position(&self, trade_params: &TradeParams) -> Result<()> {
        tracing::info!("Opening position");

        let (margin_coordinator, margin_trader) = get_margins(trade_params);
        let total_collateral = margin_coordinator + margin_trader;
        let (leverage_long, leverage_short) = get_leverages(trade_params);

        let contract_descriptor = build_contract_descriptor(
            total_collateral,
            trade_params.execution_price,
            leverage_long,
            leverage_short,
        )?;

        let contract_symbol = trade_params.contract_symbol.label();
        let maturity_time = trade_params.expiry_timestamp;

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator
        let contract_input = ContractInput {
            offer_collateral: margin_trader,
            accept_collateral: margin_coordinator,
            fee_rate: 2,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![self.inner.oracle_pk()],
                    event_id: format!("{contract_symbol}{maturity_time}"),
                    threshold: 1,
                },
            }],
        };

        let channel_details = self.get_counterparty_channel(trade_params.pubkey)?;
        self.inner
            .propose_dlc_channel(&channel_details, &contract_input)
            .await?;
        Ok(())
    }

    fn get_counterparty_channel(&self, trader_pubkey: PublicKey) -> Result<ChannelDetails> {
        let channel_details = self.inner.list_usable_channels();
        let channel_details = channel_details
            .into_iter()
            .find(|c| c.counterparty.node_id == trader_pubkey)
            .context("Channel details not found")
            .map_err(|e| anyhow!("{e:#}"))?;
        Ok(channel_details)
    }
}

fn get_margins(trade_params: &TradeParams) -> (u64, u64) {
    let margin_coordinator = calculate_margin(
        trade_params.execution_price,
        trade_params.quantity,
        COORDINATOR_LEVERAGE,
    );
    let margin_trader = calculate_margin(
        trade_params.execution_price,
        trade_params.quantity,
        trade_params.leverage,
    );

    (margin_coordinator, margin_trader)
}

fn get_leverages(trade_params: &TradeParams) -> (f64, f64) {
    match trade_params.direction {
        Direction::Long => (trade_params.leverage, COORDINATOR_LEVERAGE),
        Direction::Short => (COORDINATOR_LEVERAGE, trade_params.leverage),
    }
}

fn get_rounding_intervals() -> RoundingIntervals {
    RoundingIntervals {
        intervals: vec![RoundingInterval {
            begin_interval: 0,
            rounding_mod: 500,
        }],
    }
}

/// Builds the contract descriptor from the point of view of the trader.
fn build_contract_descriptor(
    total_collateral: u64,
    execution_price: f64,
    leverage_long: f64,
    leverage_short: f64,
) -> Result<ContractDescriptor> {
    Ok(ContractDescriptor::Numerical(NumericalDescriptor {
        payout_function: build_payout_curve(
            total_collateral,
            execution_price,
            leverage_long,
            leverage_short,
        )?,
        rounding_intervals: get_rounding_intervals(),
        difference_params: None,
        oracle_numeric_infos: dlc_trie::OracleNumericInfo {
            base: 2,
            nb_digits: vec![20],
        },
    }))
}

/// Builds the payout curve
///
/// todo: this method is currently using a linear payout curve provided by the `rust-dlc`
/// `PayoutFunction`. Replace with our own inverse payout curve.
fn build_payout_curve(
    total_collateral: u64,
    execution_price: f64,
    leverage_long: f64,
    leverage_short: f64,
) -> Result<PayoutFunction> {
    let leverage_short = Decimal::try_from(leverage_short)?;
    let execution_price = Decimal::try_from(execution_price)?;
    let liquidation_price_short =
        calculate_short_liquidation_price(leverage_short, execution_price);

    let leverage_long = Decimal::try_from(leverage_long)?;
    let liquidation_price_long = calculate_long_liquidation_price(leverage_long, execution_price);

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
            outcome_payout: 0,
            extra_precision: 0,
        },
        PayoutPoint {
            event_outcome: lower_limit,
            outcome_payout: 0,
            extra_precision: 0,
        },
    ])
    .map_err(|e| anyhow!("{e:#}"))?;

    let middle_range = PolynomialPayoutCurvePiece::new(vec![
        PayoutPoint {
            event_outcome: lower_limit,
            outcome_payout: 0,
            extra_precision: 0,
        },
        PayoutPoint {
            event_outcome: upper_limit,
            outcome_payout: total_collateral,
            extra_precision: 0,
        },
    ])
    .map_err(|e| anyhow!("{e:#}"))?;

    let upper_range = PolynomialPayoutCurvePiece::new(vec![
        PayoutPoint {
            event_outcome: upper_limit,
            outcome_payout: total_collateral,
            extra_precision: 0,
        },
        PayoutPoint {
            // todo: this number is copied from the rust-dlc examples and is probably
            // chosen randomly. pick a sensible number for this upper range value.
            event_outcome: 1048575,
            outcome_payout: total_collateral,
            extra_precision: 0,
        },
    ])
    .map_err(|e| anyhow!("{e:#}"))?;

    PayoutFunction::new(vec![
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(lower_range),
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(middle_range),
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(upper_range),
    ])
    .map_err(|e| anyhow!("{e:#}"))
}
