use crate::dlc;
use anyhow::Context;
use anyhow::Result;

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
    /// The leverage the coordinator will take
    pub coordinator_leverage: f32,
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
    let coordinator_leverage = option.coordinator_leverage;

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
                coordinator_leverage,
                min_quantity,
                is_channel_balance: false,
                min_margin,
                maintenance_margin_rate,
                order_matching_fee_rate,
                total_collateral: None,
            }
        }
        Some(channel) => {
            let local_balance = dlc::get_usable_dlc_channel_balance()?.to_sat();
            let counterparty_balance = dlc::get_usable_dlc_channel_balance_counterparty()?.to_sat();

            TradeConstraints {
                max_local_balance_sats: local_balance,
                max_counterparty_balance_sats: counterparty_balance,
                coordinator_leverage,
                min_quantity,
                is_channel_balance: true,
                min_margin,
                maintenance_margin_rate,
                order_matching_fee_rate,
                total_collateral: Some(
                    channel.own_params.collateral + channel.counter_params.collateral,
                ),
            }
        }
    };
    Ok(trade_constraints)
}
