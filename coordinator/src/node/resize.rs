use crate::compute_relative_contracts;
use crate::db;
use crate::decimal_from_f32;
use crate::f32_from_decimal;
use crate::node::Node;
use crate::payout_curve;
use crate::payout_curve::create_rounding_interval;
use crate::position::models::Position;
use crate::position::models::PositionState;
use crate::trade::models::NewTrade;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use commons::order_matching_fee_taker;
use commons::TradeParams;
use diesel::Connection;
use diesel::PgConnection;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use lightning::ln::ChannelId;
use rust_decimal::prelude::Signed;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use tokio::task::block_in_place;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;

impl Node {
    pub async fn resize_position(
        &self,
        conn: &mut PgConnection,
        channel_id: ChannelId,
        trade_params: &TradeParams,
    ) -> Result<()> {
        let channel_id_hex = channel_id.to_hex();
        let peer_id = trade_params.pubkey;

        let position = db::positions::Position::get_position_by_trader(
            conn,
            peer_id,
            vec![PositionState::Open],
        )?
        .with_context(|| {
            format!("Failed to find open position for channel {channel_id_hex} with peer {peer_id}")
        })?;

        tracing::info!(
            ?position,
            ?trade_params,
            channel_id = %channel_id_hex,
            peer_id = %peer_id,
            "Resizing position",
        );

        let runtime_handle = tokio::runtime::Handle::current();
        block_in_place(|| {
            self.start_position_resizing(
                conn,
                runtime_handle,
                channel_id,
                trade_params,
                peer_id,
                position,
            )
        })?;

        Ok(())
    }

    /// Start the position resizing protocol by first closing the corresponding subchannel.
    fn start_position_resizing(
        &self,
        conn: &mut PgConnection,
        runtime: tokio::runtime::Handle,
        channel_id: ChannelId,
        trade_params: &TradeParams,
        peer_id: PublicKey,
        position: Position,
    ) -> Result<(), anyhow::Error> {
        conn.transaction::<(), _, _>(|tx| {
            self.start_position_resizing_aux(
                tx,
                runtime,
                channel_id,
                trade_params,
                peer_id,
                position,
            )
            .map_err(|e| {
                tracing::error!("Failed to start position resizing: {e:#}");

                // We map all errors to `RollbackTransaction` because we want to ensure that our
                // database transaction is only committed if the resize protocol has started
                // correctly.
                diesel::result::Error::RollbackTransaction
            })
        })?;

        Ok(())
    }

    /// Auxiliary method to start the position resizing protocol.
    ///
    /// This allows us to return `anyhow::Result`.
    fn start_position_resizing_aux(
        &self,
        tx: &mut PgConnection,
        runtime: tokio::runtime::Handle,
        channel_id: ChannelId,
        trade_params: &TradeParams,
        peer_id: PublicKey,
        position: Position,
    ) -> Result<()> {
        db::positions::Position::set_open_position_to_resizing(tx, position.trader.to_string())
            .context("Could not update database and set position to resize")?;

        let execution_price = trade_params
            .average_execution_price()
            .to_f32()
            .expect("To fit into f32");

        // This is pretty meaningless as documented in `NewTrade`.
        let margin_coordinator = {
            let leverage_coordinator = self.coordinator_leverage_for_trade(&peer_id)?;

            margin_coordinator(trade_params, leverage_coordinator) as i64
        };

        db::trades::insert(
            tx,
            NewTrade {
                position_id: position.id,
                contract_symbol: position.contract_symbol,
                trader_pubkey: position.trader,
                quantity: trade_params.quantity,
                trader_leverage: trade_params.leverage,
                coordinator_margin: margin_coordinator,
                direction: trade_params.direction,
                average_price: execution_price,
                dlc_expiry_timestamp: Some(trade_params.filled_with.expiry_timestamp),
            },
        )?;

        runtime.block_on(async {
            let accept_settlement_amount =
                position.calculate_accept_settlement_amount_partial_close(trade_params)?;

            self.inner
                .propose_sub_channel_collaborative_settlement(
                    channel_id,
                    accept_settlement_amount.to_sat(),
                )
                .await?;

            anyhow::Ok(())
        })?;

        Ok(())
    }

    pub fn continue_position_resizing(
        &self,
        peer_id: PublicKey,
        old_position: Position,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;

        // If we have a `Resizing` position, we must be in the middle of the resizing protocol.
        // We have just finished closing the existing channel and are now ready to propose the
        // creation of the new one.
        let channel_details = self.get_counterparty_channel(peer_id)?;

        tracing::info!(
            channel_id = %channel_details.channel_id.to_hex(),
            peer_id = %peer_id,
            "Continue resizing position"
        );

        let trade = db::trades::get_latest_for_position(&mut conn, old_position.id)?
            .context("No trade for resized position")?;

        // Compute absolute contracts using formula.
        let (total_contracts, direction) = {
            let contracts_before_relative = compute_relative_contracts(
                decimal_from_f32(old_position.quantity),
                &old_position.direction,
            );
            let contracts_trade_relative =
                compute_relative_contracts(decimal_from_f32(trade.quantity), &trade.direction);

            let total_contracts_relative = contracts_before_relative + contracts_trade_relative;

            let direction = if total_contracts_relative.signum() == Decimal::ONE {
                Direction::Long
            } else {
                Direction::Short
            };

            let total_contracts = total_contracts_relative.abs();

            (total_contracts, direction)
        };

        let average_execution_price = compute_average_execution_price(
            old_position.average_entry_price,
            trade.average_price,
            old_position.quantity,
            trade.quantity,
            old_position.direction,
            trade.direction,
        );

        // NOTE: Leverage does not change with new orders!
        let leverage_trader = decimal_from_f32(old_position.trader_leverage);
        let leverage_coordinator = decimal_from_f32(old_position.coordinator_leverage);

        let margin_coordinator = compute_margin(
            total_contracts,
            leverage_coordinator,
            average_execution_price,
        );
        let margin_trader =
            compute_margin(total_contracts, leverage_trader, average_execution_price);

        let liquidation_price_trader =
            compute_liquidation_price(leverage_trader, average_execution_price, &direction);
        let expiry_timestamp = trade
            .dlc_expiry_timestamp
            .context("No expiry timestamp for resizing trade")?;

        let total_contracts = f32_from_decimal(total_contracts);
        let leverage_coordinator = f32_from_decimal(leverage_coordinator);
        let leverage_trader = f32_from_decimal(leverage_trader);

        let contract_input = {
            let fee_rate = self.settings.blocking_read().contract_tx_fee_rate;

            let contract_symbol = old_position.contract_symbol;
            let maturity_time = expiry_timestamp.unix_timestamp();
            let event_id = format!("{contract_symbol}{maturity_time}");

            let total_collateral = margin_coordinator + margin_trader;

            let coordinator_direction = direction.opposite();

            // Apply the order-matching fee. The fee from the previous iteration of the position
            // should have already been cashed into the coordinator's side of the Lightning
            // channel when first closing the DLC channel.
            //
            // Here we only need to charge for executing the order.
            let fee =
                order_matching_fee_taker(trade.quantity, decimal_from_f32(trade.average_price))
                    .to_sat();

            let contract_descriptor = payout_curve::build_contract_descriptor(
                average_execution_price,
                margin_coordinator,
                margin_trader,
                leverage_coordinator,
                leverage_trader,
                coordinator_direction,
                fee,
                create_rounding_interval(total_collateral),
                total_contracts,
                contract_symbol,
            )
            .context("Could not build contract descriptor")?;

            tracing::info!(
                channel_id = %channel_details.channel_id.to_hex(),
                peer_id = %peer_id,
                event_id,
                "Proposing DLC channel as part of position resizing"
            );

            ContractInput {
                offer_collateral: margin_coordinator - fee,
                // the accepting party has do bring in additional margin for the fees
                accept_collateral: margin_trader + fee,
                fee_rate,
                contract_infos: vec![ContractInputInfo {
                    contract_descriptor,
                    oracles: OracleInput {
                        public_keys: vec![self.inner.oracle_pubkey],
                        event_id,
                        threshold: 1,
                    },
                }],
            }
        };

        tokio::spawn({
            let node = self.inner.clone();
            async move {
                if let Err(e) = node
                    .propose_sub_channel(channel_details.clone(), contract_input)
                    .await
                {
                    tracing::error!(
                        channel_id = %channel_details.channel_id.to_hex(),
                        peer_id = %peer_id,
                        "Failed to propose DLC channel as part of position resizing: {e:#}"
                    );
                    return;
                }

                let temporary_contract_id = match node
                    .get_temporary_contract_id_by_sub_channel_id(channel_details.channel_id)
                {
                    Ok(temporary_contract_id) => temporary_contract_id,
                    Err(e) => {
                        tracing::error!(
                            channel_id = %channel_details.channel_id.to_hex(),
                            "Unable to extract temporary contract id: {e:#}"
                        );
                        return;
                    }
                };

                // TODO: We are too eager to update the position as the protocol is not quite
                // done. We should use a separate table that holds all the information needed to
                // update the position once the resize protocol is actually done.
                if let Err(e) = db::positions::Position::update_resized_position(
                    &mut conn,
                    old_position.trader.to_string(),
                    total_contracts,
                    direction.into(),
                    leverage_coordinator,
                    leverage_trader,
                    margin_coordinator as i64,
                    margin_trader as i64,
                    f32_from_decimal(average_execution_price),
                    f32_from_decimal(liquidation_price_trader),
                    expiry_timestamp,
                    temporary_contract_id,
                ) {
                    tracing::error!(
                        channel_id = %channel_details.channel_id.to_hex(),
                        "Failed to update resized position: {e:#}"
                    )
                }
            }
        });

        Ok(())
    }
}

fn margin_coordinator(trade_params: &TradeParams, coordinator_leverage: f32) -> u64 {
    calculate_margin(
        trade_params.average_execution_price(),
        trade_params.quantity,
        coordinator_leverage,
    )
}

fn compute_margin(
    total_contracts: Decimal,
    leverage: Decimal,
    average_execution_price: Decimal,
) -> u64 {
    let margin_btc = total_contracts / (leverage * average_execution_price);

    let margin_btc = margin_btc
        .abs()
        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
        .to_f64()
        .expect("margin to fit into f64");

    bitcoin::Amount::from_btc(margin_btc)
        .expect("margin to fit into Amount")
        .to_sat()
}

fn compute_average_execution_price(
    starting_average_execution_price: f32,
    trade_execution_price: f32,
    starting_contracts: f32,
    trade_contracts: f32,
    starting_direction: Direction,
    trade_direction: Direction,
) -> Decimal {
    let starting_average_execution_price = decimal_from_f32(starting_average_execution_price);

    let trade_execution_price = decimal_from_f32(trade_execution_price);

    let starting_contracts = decimal_from_f32(starting_contracts);
    let starting_contracts_relative =
        compute_relative_contracts(starting_contracts, &starting_direction);

    let trade_contracts = decimal_from_f32(trade_contracts);
    let trade_contracts_relative = compute_relative_contracts(trade_contracts, &trade_direction);

    let total_contracts_relative = starting_contracts_relative + trade_contracts_relative;

    // If the position size has reduced, the average execution price does not change.
    if starting_contracts_relative.signum() == total_contracts_relative.signum()
        && starting_contracts > total_contracts_relative.abs()
    {
        starting_average_execution_price
    }
    // If the position size has increased, the average execution price is updated.
    else if starting_contracts_relative.signum() == total_contracts_relative.signum()
        && starting_contracts < total_contracts_relative.abs()
    {
        let price = total_contracts_relative
            / (starting_contracts_relative / starting_average_execution_price
                + trade_contracts_relative / trade_execution_price);

        price.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
    }
    // If the position has changed direction, the average execution price is the new trade execution
    // price.
    else if starting_contracts_relative.signum() != total_contracts_relative.signum() {
        trade_execution_price
    }
    // If the position hasn't changed (same direction, same number of contracts), the average
    // execution price does not change. But why are we even here?
    else {
        debug_assert!(false);
        starting_average_execution_price
    }
}

fn compute_liquidation_price(leverage: Decimal, price: Decimal, direction: &Direction) -> Decimal {
    match direction {
        Direction::Long => calculate_long_liquidation_price(leverage, price),
        Direction::Short => calculate_short_liquidation_price(leverage, price),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn average_execution_price_extend_position() {
        let price = compute_average_execution_price(
            10_000.0,
            30_000.0,
            10.0,
            10.0,
            Direction::Long,
            Direction::Long,
        );

        assert_eq!(price, dec!(15_000));
    }

    #[test]
    fn average_execution_price_reduce_position() {
        let price = compute_average_execution_price(
            10_000.0,
            20_000.0,
            10.0,
            5.0,
            Direction::Long,
            Direction::Short,
        );

        assert_eq!(price, dec!(10_000));
    }

    #[test]
    fn average_execution_price_change_position_direction() {
        let price = compute_average_execution_price(
            10_000.0,
            20_000.0,
            10.0,
            15.0,
            Direction::Long,
            Direction::Short,
        );

        assert_eq!(price, dec!(20_000));
    }
}
