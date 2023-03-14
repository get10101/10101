use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use coordinator_commons::TradeParams;
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
use dlc_manager::ChannelId;
use lightning::ln::channelmanager::ChannelDetails;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;

/// The leverage used by the coordinator for all trades.
///
/// TODO: In case of leverage 1.0 the liquidation will be set to
/// 21_000_000. I guess this will result in issues when the overall
/// upper boundary is smaller than that. It looks like the payout
/// function will never finish in that case. Setting this value
/// temporarily to 2.0 so that the liquidation price is below that
/// total upper boundary.
const COORDINATOR_LEVERAGE: f64 = 2.0;

pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
}

impl Node {
    pub async fn trade(&self, trade_params: &TradeParams) -> Result<()> {
        match self.decide_trade_action(trade_params)? {
            TradeAction::Open => self.open_position(trade_params).await?,
            TradeAction::Close(channel_id) => self.close_position(trade_params, channel_id).await?,
        };

        Ok(())
    }

    async fn open_position(&self, trade_params: &TradeParams) -> Result<()> {
        tracing::info!("Opening position");

        let margin_trader = margin_trader(trade_params);
        let margin_coordinator = margin_coordinator(trade_params);

        let leverage_long = leverage_long(trade_params);
        let leverage_short = leverage_short(trade_params);

        let total_collateral = margin_coordinator + margin_trader;

        let contract_descriptor = build_contract_descriptor(
            total_collateral,
            trade_params.weighted_execution_price(),
            leverage_long,
            leverage_short,
        )?;

        let contract_symbol = trade_params.contract_symbol.label();
        let maturity_time = trade_params.filled_with.expiry_timestamp;

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

    async fn close_position(
        &self,
        trade_params: &TradeParams,
        channel_id: ChannelId,
    ) -> Result<()> {
        let trader_pk = trade_params.pubkey;

        tracing::info!(
            order_id = %trade_params.filled_with.order_id,
            %trader_pk,
            "Closing position"
        );

        let margin_trader = margin_trader(trade_params);
        let margin_coordinator = margin_coordinator(trade_params);

        let leverage_long = leverage_long(trade_params);
        let leverage_short = leverage_short(trade_params);

        let total_collateral = margin_coordinator + margin_trader;

        let payout_function = build_payout_curve(
            total_collateral,
            trade_params.weighted_execution_price(),
            leverage_long,
            leverage_short,
        )?;

        let accept_settlement_amount = payout_function
            .to_range_payouts(total_collateral, &get_rounding_intervals())
            .map_err(|e| anyhow!("{e:#}"))?
            .iter()
            .find(|p| trade_params.weighted_execution_price() < (p.start + p.count) as f64)
            .map(|p| p.payout.accept)
            .context("Failed to find payout.")?;

        tracing::debug!(
            "Settling position of {accept_settlement_amount} with {}",
            trade_params.pubkey
        );

        self.inner
            .propose_dlc_channel_collaborative_settlement(&channel_id, accept_settlement_amount)?;

        Ok(())
    }

    /// Decides what trade action should be performed according to the
    /// coordinator's current trading status with the trader.
    ///
    /// We look for a pre-existing position with the trader and
    /// instruct accordingly:
    ///
    /// 1. If a position of equal quantity and opposite direction is
    /// found, we direct the caller to close the position.
    ///
    /// 2. If no position is found, we direct the caller to open a
    /// position.
    ///
    /// 3. If a position of differing quantity is found, we direct the
    /// caller to extend or reduce the position. _This is currently
    /// not supported_.
    fn decide_trade_action(&self, trade_params: &TradeParams) -> Result<TradeAction> {
        let action = match self.inner.get_sub_channel_signed(&trade_params.pubkey)? {
            Some(subchannel) => {
                // FIXME: Should query the database for more
                // information

                // TODO: Detect if the position should be
                // extended/reduced. Return corresponding error as
                // this is currently not supported.

                TradeAction::Close(subchannel.channel_id)
            }
            None => TradeAction::Open,
        };

        Ok(action)
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

enum TradeAction {
    Open,
    Close(ChannelId),
    // Extend,
    // Reduce,
}

fn margin_trader(trade_params: &TradeParams) -> u64 {
    calculate_margin(
        trade_params.weighted_execution_price(),
        trade_params.quantity,
        trade_params.leverage,
    )
}

fn margin_coordinator(trade_params: &TradeParams) -> u64 {
    calculate_margin(
        trade_params.weighted_execution_price(),
        trade_params.quantity,
        COORDINATOR_LEVERAGE,
    )
}

fn leverage_long(trade_params: &TradeParams) -> f64 {
    match trade_params.direction {
        Direction::Long => trade_params.leverage,
        Direction::Short => COORDINATOR_LEVERAGE,
    }
}

fn leverage_short(trade_params: &TradeParams) -> f64 {
    match trade_params.direction {
        Direction::Long => COORDINATOR_LEVERAGE,
        Direction::Short => trade_params.leverage,
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

/// Builds a [`PayoutFunction`].
///
/// TODO: We are currently building a linear payout function for
/// simplicity. This is *wrong*. We should build an inverse payout
/// function like we used to do in ItchySats.
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
            // TODO: This number is copied from the rust-dlc examples and is probably
            // chosen randomly. Pick a sensible number for this upper range value.
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
