use crate::api::TradeConstraints;
use crate::ln_dlc;
use anyhow::Context;
use anyhow::Result;

pub fn channel_trade_constraints() -> Result<TradeConstraints> {
    let lsp_config =
        crate::state::try_get_lsp_config().context("We can't trade without LSP config")?;

    let signed_channel = ln_dlc::get_signed_dlc_channel()?;

    // TODO(bonomat): retrieve these values from the coordinator. This can come from the liquidity
    // options.
    let min_quantity = signed_channel.map(|_| 100).unwrap_or(500);

    // TODO(bonomat): this logic should be removed once we have our liquidity options again and the
    // on-boarding logic. For now we take the highest liquidity option
    let option = lsp_config
        .liquidity_options
        .iter()
        .filter(|option| option.active)
        .max_by_key(|option| &option.trade_up_to_sats)
        .context("we need at least one liquidity option")?;
    let coordinator_leverage = option.coordinator_leverage;

    let dlc_channels = ln_dlc::get_signed_dlc_channels()?;

    let maybe_channel = dlc_channels.first();

    let trade_constraints = match maybe_channel {
        None => {
            let balance = ln_dlc::get_onchain_balance()?;
            let counterparty_margin_sats = option.trade_up_to_sats;
            TradeConstraints {
                max_local_margin_sats: balance.confirmed
                    + balance.trusted_pending
                    + balance.untrusted_pending,

                max_counterparty_margin_sats: counterparty_margin_sats,
                coordinator_leverage,
                min_quantity,
                is_channel_balance: false,
            }
        }
        Some(channel) => TradeConstraints {
            max_local_margin_sats: ln_dlc::get_usable_dlc_channel_balance()?.to_sat(),
            max_counterparty_margin_sats: channel.counter_params.collateral,
            coordinator_leverage,
            min_quantity,
            is_channel_balance: true,
        },
    };
    Ok(trade_constraints)
}
