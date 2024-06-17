use crate::dlc;
use anyhow::Context;
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;

pub struct TradeConstraints {
    /// Max balance the local party can use
    ///
    /// This depends on whether the user has a channel or not. If he has a channel, then his
    /// channel balance is the max amount, otherwise his on-chain balance dictates the max amount
    pub max_local_balance_sats: u64,
    /// Max amount the counterparty is willing to put.
    ///
    /// This depends whether the user has a channel or not, i.e. if he has a channel then the max
    /// amount is what the counterparty has in the channel, otherwise, it's a fixed amount what
    /// the counterparty is willing to provide.
    pub max_counterparty_balance_sats: u64,
    /// Smallest allowed amount of contracts
    pub min_quantity: u64,
    /// If true it means that the user has a channel and hence the max amount is limited by what he
    /// has in the channel. In the future we can consider splice in and allow the user to use more
    /// than just his channel balance.
    pub is_channel_balance: bool,
    /// Smallest allowed margin
    pub min_margin: u64,
    /// The maintenance margin in percent defines the margin requirement left in the dlc channel.
    /// If the margin drops below that value the position gets liquidated.
    pub maintenance_margin_rate: f32,
    /// The fee rate for order matching.
    pub order_matching_fee_rate: f32,
    /// Total collateral in the dlc channel, none if [`is_channel_balance`] is false.
    pub total_collateral: Option<u64>,
    /// Defines what leverage the coordinator will take depending on what the trader takes
    ///
    /// Unfortunately our version of flutter_rust_bridge does not support hashmaps yet
    pub coordinator_leverages: Vec<CoordinatorLeverage>,
    /// The leverage the coordinator will take if none is provided in `coordinator_leverages`
    /// TODO(bonomat): we should introduce a separate leverage/multiplier to derive channel sizes
    pub default_coordinator_leverage: u8,
}

/// Trader/Coordinator leverage pair
///
/// The
pub struct CoordinatorLeverage {
    pub trader_leverage: u8,
    pub coordinator_leverage: u8,
}

impl TradeConstraints {
    /// looks up coordinator leverage in `coordinator_leverages` and falls back to
    /// `default_coordinator_leverage` if none was found
    pub fn coordinator_leverage(&self, trader_leverage: u8) -> f32 {
        self.coordinator_leverages
            .iter()
            .find_map(|leverage| {
                if leverage.trader_leverage == trader_leverage {
                    Some(leverage.coordinator_leverage)
                } else {
                    None
                }
            })
            .unwrap_or(self.default_coordinator_leverage)
            .to_f32()
            .expect("to fit")
    }

    /// looks up coordinator leverage in `coordinator_leverages` and falls back to
    /// `default_coordinator_leverage` if none was found.
    ///
    /// rounds `trader_leverage` up.
    pub fn coordinator_leverage_by_f32(&self, trader_leverage: f32) -> f32 {
        self.coordinator_leverage(trader_leverage.round().to_u8().expect("to fit"))
    }
}

pub fn channel_trade_constraints() -> Result<TradeConstraints> {
    let config =
        crate::state::try_get_tentenone_config().context("We can't trade without LSP config")?;

    let signed_channel = dlc::get_signed_dlc_channel()?;

    let min_margin = match &signed_channel {
        Some(_) => 1,
        // TODO(holzeis): https://github.com/get10101/10101/issues/1905
        None => 250_000,
    };

    let min_quantity = config.min_quantity;
    let maintenance_margin_rate = config.maintenance_margin_rate;
    let order_matching_fee_rate = config.order_matching_fee_rate;

    // TODO(bonomat): this logic should be removed once we have our liquidity options again and the
    // on-boarding logic. For now we take the highest liquidity option
    let option = config
        .liquidity_options
        .iter()
        .filter(|option| option.active)
        .max_by_key(|option| &option.trade_up_to_sats)
        .context("we need at least one liquidity option")?;
    let default_coordinator_leverage = config.default_coordinator_leverage;

    let coordinator_leverages = config
        .coordinator_leverages
        .into_iter()
        .map(|(trader, coordinator)| CoordinatorLeverage {
            trader_leverage: trader,
            coordinator_leverage: coordinator,
        })
        .collect();

    // FIXME: This doesn't work if the channel is in `Closing` and related states.
    let trade_constraints = match signed_channel {
        None => {
            let balance = dlc::get_onchain_balance();
            let counterparty_balance_sats = option.trade_up_to_sats;

            TradeConstraints {
                max_local_balance_sats: balance.confirmed
                    + balance.trusted_pending
                    + balance.untrusted_pending,
                max_counterparty_balance_sats: counterparty_balance_sats,
                coordinator_leverages,
                min_quantity,
                is_channel_balance: false,
                min_margin,
                maintenance_margin_rate,
                order_matching_fee_rate,
                total_collateral: None,
                default_coordinator_leverage,
            }
        }
        Some(channel) => {
            let local_balance = dlc::get_usable_dlc_channel_balance()?.to_sat();
            let counterparty_balance = dlc::get_usable_dlc_channel_balance_counterparty()?.to_sat();

            TradeConstraints {
                max_local_balance_sats: local_balance,
                max_counterparty_balance_sats: counterparty_balance,
                coordinator_leverages,
                min_quantity,
                is_channel_balance: true,
                min_margin,
                maintenance_margin_rate,
                order_matching_fee_rate,
                total_collateral: Some(
                    channel.own_params.collateral + channel.counter_params.collateral,
                ),
                default_coordinator_leverage,
            }
        }
    };
    Ok(trade_constraints)
}
