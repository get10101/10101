use crate::compute_relative_contracts;
use crate::decimal_from_f32;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Txid;
use commons::TradeParams;
use dlc_manager::ContractId;
use dlc_manager::DlcChannelId;
use lightning::ln::ChannelId;
use rust_decimal::prelude::Signed;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::bitmex_client::Quote;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_pnl;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Clone)]
pub struct NewPosition {
    pub contract_symbol: ContractSymbol,
    pub trader_leverage: f32,
    pub quantity: f32,
    pub trader_direction: Direction,
    pub trader: PublicKey,
    pub average_entry_price: f32,
    pub trader_liquidation_price: Decimal,
    pub coordinator_liquidation_price: Decimal,
    pub coordinator_margin: i64,
    pub expiry_timestamp: OffsetDateTime,
    pub temporary_contract_id: ContractId,
    pub coordinator_leverage: f32,
    pub trader_margin: i64,
    pub stable: bool,
    pub order_matching_fees: Amount,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PositionState {
    /// The position is in the process of being opened.
    ///
    /// Once the position is fully opened it will end in the state `Open`
    Proposed,
    Open,
    /// The position is in the process of being closed.
    ///
    /// Once the position is being closed the closing price is known.
    Closing {
        closing_price: f32,
    },
    Closed {
        pnl: i64,
    },
    /// The position was not opened successfully.
    Failed,
    Rollover,
    Resizing,
}

/// The trading position for a user identified by `trader`.
#[derive(Clone)]
pub struct Position {
    pub id: i32,
    pub trader: PublicKey,
    pub contract_symbol: ContractSymbol,
    pub quantity: f32,
    pub trader_direction: Direction,

    pub average_entry_price: f32,
    pub closing_price: Option<f32>,
    pub trader_realized_pnl_sat: Option<i64>,

    pub trader_liquidation_price: f32,
    pub coordinator_liquidation_price: f32,

    pub trader_margin: i64,
    pub coordinator_margin: i64,

    pub trader_leverage: f32,
    pub coordinator_leverage: f32,

    pub position_state: PositionState,

    /// Accumulated order matching fees for the lifetime of the position.
    pub order_matching_fees: Amount,

    pub creation_timestamp: OffsetDateTime,
    pub expiry_timestamp: OffsetDateTime,
    pub update_timestamp: OffsetDateTime,

    /// The temporary contract ID that is created when an [`OfferedContract`] is sent.
    ///
    /// We use the temporary contract ID because the actual contract ID is not always available.
    /// The temporary contract ID is propagated to all `rust-dlc` states until the contract is
    /// closed.
    ///
    /// This field is optional to maintain backwards compatibility, because we cannot
    /// deterministically associate already existing contracts with positions.
    ///
    /// [`OfferedContract`]: dlc_manager::contract::offered_contract::OfferedContract
    pub temporary_contract_id: Option<ContractId>,

    pub stable: bool,
}

impl Position {
    // Returns true if the position is expired
    pub fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() >= self.expiry_timestamp
    }

    /// Calculates the profit and loss for the coordinator in satoshis
    pub fn calculate_coordinator_pnl(&self, quote: Quote) -> Result<i64> {
        let closing_price = match self.closing_price {
            None => quote.get_price_for_direction(self.trader_direction.opposite()),
            Some(closing_price) => {
                Decimal::try_from(closing_price).expect("f32 closing price to fit into decimal")
            }
        };

        let average_entry_price = Decimal::try_from(self.average_entry_price)
            .context("Failed to convert average entry price to Decimal")?;

        let (long_leverage, short_leverage) = match self.trader_direction {
            Direction::Long => (self.trader_leverage, self.coordinator_leverage),
            Direction::Short => (self.coordinator_leverage, self.trader_leverage),
        };

        let direction = self.trader_direction.opposite();

        let long_margin = calculate_margin(average_entry_price, self.quantity, long_leverage);
        let short_margin = calculate_margin(average_entry_price, self.quantity, short_leverage);

        let pnl = calculate_pnl(
            average_entry_price,
            closing_price,
            self.quantity,
            direction,
            long_margin,
            short_margin,
        )
        .context("Failed to calculate pnl for position")?;

        Ok(pnl)
    }

    /// Calculate the settlement amount for the coordinator when closing the _entire_ position.
    pub fn calculate_coordinator_settlement_amount(
        &self,
        closing_price: Decimal,
        matching_fee: Amount,
    ) -> Result<u64> {
        let opening_price = Decimal::try_from(self.average_entry_price)?;

        let leverage_long = leverage_long(
            self.trader_direction,
            self.trader_leverage,
            self.coordinator_leverage,
        );
        let leverage_short = leverage_short(
            self.trader_direction,
            self.trader_leverage,
            self.coordinator_leverage,
        );

        let coordinator_direction = self.trader_direction.opposite();
        calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            self.quantity,
            leverage_long,
            leverage_short,
            coordinator_direction,
            matching_fee,
        )
    }

    /// Calculate the settlement amount for the accept party (i.e. the trader) when closing the DLC
    /// channel for the two-step position resizing protocol.
    pub fn calculate_accept_settlement_amount_partial_close(
        &self,
        trade_params: &TradeParams,
    ) -> Result<Amount> {
        calculate_accept_settlement_amount_partial_close(
            self.quantity,
            self.trader_direction,
            self.average_entry_price,
            self.trader_leverage,
            self.coordinator_leverage,
            trade_params.quantity,
            trade_params.direction,
            trade_params.average_execution_price(),
        )
    }
}

/// Calculate the settlement amount for the coordinator, based on the PNL and the order-matching
/// closing fee.
fn calculate_coordinator_settlement_amount(
    opening_price: Decimal,
    closing_price: Decimal,
    quantity: f32,
    long_leverage: f32,
    short_leverage: f32,
    coordinator_direction: Direction,
    matching_fee: Amount,
) -> Result<u64> {
    let close_position_fee = matching_fee.to_sat();

    let long_margin = calculate_margin(opening_price, quantity, long_leverage);
    let short_margin = calculate_margin(opening_price, quantity, short_leverage);
    let total_margin = long_margin + short_margin;

    let pnl = calculate_pnl(
        opening_price,
        closing_price,
        quantity,
        coordinator_direction,
        long_margin,
        short_margin,
    )?;

    let coordinator_margin = match coordinator_direction {
        Direction::Long => long_margin,
        Direction::Short => short_margin,
    };

    let coordinator_settlement_amount = Decimal::from(coordinator_margin) + Decimal::from(pnl);

    // Double-checking that the coordinator's payout isn't negative, although `calculate_pnl` should
    // guarantee this.
    let coordinator_settlement_amount = coordinator_settlement_amount.max(Decimal::ZERO);

    // The coordinator should always get at least the order-matching fee for closing the position.
    let coordinator_settlement_amount =
        coordinator_settlement_amount + Decimal::from(close_position_fee);

    let coordinator_settlement_amount = coordinator_settlement_amount
        .to_u64()
        .expect("to fit into u64");

    // The coordinator's maximum settlement amount is capped by the total combined margin in the
    // contract.
    let coordinator_settlement_amount = coordinator_settlement_amount.min(total_margin);

    Ok(coordinator_settlement_amount)
}

/// Calculate the settlement amount for the accept party (i.e. the trader) when closing the DLC
/// channel for the two-step position resizing protocol.
///
/// There are 3 distinct cases:
///
/// 1. The position is reduced: settle current DLC channel at `original_margin + PNL` based on
/// the number of contracts removed and the order's execution price.
///
/// 2. The position flips direction: settle the current DLC channel at `original_margin + PNL`
/// based on the number of contracts removed (the whole position) and the order's execution
/// price.
///
/// 3. The position is extended: settle the current DLC channel at `original_margin`. Nothing
/// has been actually settled in terms of the position, so we just want to remake the channel
/// with more contracts.
///
/// NOTE: The `position.trader_margin` has already been subtracted the previous order-matching
/// fee, so we don't have to do anything about that.
#[allow(clippy::too_many_arguments)]
fn calculate_accept_settlement_amount_partial_close(
    position_quantity: f32,
    position_direction: Direction,
    position_average_execution_price: f32,
    position_trader_leverage: f32,
    position_coordinator_leverage: f32,
    trade_quantity: f32,
    trade_direction: Direction,
    trade_average_execution_price: Decimal,
) -> Result<Amount> {
    let contracts_before_relative =
        compute_relative_contracts(decimal_from_f32(position_quantity), &position_direction);
    let contracts_trade_relative =
        compute_relative_contracts(decimal_from_f32(trade_quantity), &trade_direction);

    let contracts_after_relative = contracts_before_relative + contracts_trade_relative;

    let leverage_long = leverage_long(
        position_direction,
        position_trader_leverage,
        position_coordinator_leverage,
    );
    let leverage_short = leverage_short(
        position_direction,
        position_trader_leverage,
        position_coordinator_leverage,
    );

    let position_trader_margin = calculate_margin(
        decimal_from_f32(position_average_execution_price),
        position_quantity,
        position_trader_leverage,
    );

    // Position reduced.
    let settlement_amount = if contracts_before_relative.signum()
        == contracts_after_relative.signum()
        && contracts_before_relative.abs() > contracts_after_relative.abs()
        && !contracts_after_relative.is_zero()
    {
        // Settled as many contracts as there are in the executed order.
        let settled_contracts = trade_quantity;

        let opening_price = decimal_from_f32(position_average_execution_price);

        let long_margin = calculate_margin(opening_price, settled_contracts, leverage_long);
        let short_margin = calculate_margin(opening_price, settled_contracts, leverage_short);

        let pnl = calculate_pnl(
            opening_price,
            trade_average_execution_price,
            settled_contracts,
            position_direction,
            long_margin,
            short_margin,
        )?;

        ((position_trader_margin as i64) + pnl).max(0) as u64
    }
    // Position changed direction.
    else if contracts_before_relative.signum() != contracts_after_relative.signum()
        && !contracts_after_relative.is_zero()
    {
        // Settled as many contracts as there are in the entire position.
        let settled_contracts = position_quantity;

        let opening_price = decimal_from_f32(position_average_execution_price);

        let long_margin = calculate_margin(opening_price, settled_contracts, leverage_long);
        let short_margin = calculate_margin(opening_price, settled_contracts, leverage_short);

        let pnl = calculate_pnl(
            opening_price,
            trade_average_execution_price,
            settled_contracts,
            position_direction,
            long_margin,
            short_margin,
        )?;

        ((position_trader_margin as i64) + pnl).max(0) as u64
    }
    // Position extended.
    else if contracts_before_relative.signum() == contracts_after_relative.signum()
        && contracts_before_relative.abs() < contracts_after_relative.abs()
    {
        position_trader_margin
    }
    // Position either fully settled or unchanged. This is a bug.
    else {
        debug_assert!(false);

        bail!("Invalid parameters for position resizing");
    };

    let settlement_amount = Amount::from_sat(settlement_amount);

    Ok(settlement_amount)
}

pub fn leverage_long(direction: Direction, trader_leverage: f32, coordinator_leverage: f32) -> f32 {
    match direction {
        Direction::Long => trader_leverage,
        Direction::Short => coordinator_leverage,
    }
}

pub fn leverage_short(
    direction: Direction,
    trader_leverage: f32,
    coordinator_leverage: f32,
) -> f32 {
    match direction {
        Direction::Long => coordinator_leverage,
        Direction::Short => trader_leverage,
    }
}

#[derive(Clone, Debug)]
pub struct CollaborativeRevert {
    pub channel_id: DlcChannelId,
    pub trader_pubkey: PublicKey,
    pub price: Decimal,
    pub coordinator_address: Address,
    pub coordinator_amount_sats: Amount,
    pub trader_amount_sats: Amount,
    pub timestamp: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct LegacyCollaborativeRevert {
    pub channel_id: ChannelId,
    pub trader_pubkey: PublicKey,
    pub price: f32,
    pub coordinator_address: Address,
    pub coordinator_amount_sats: Amount,
    pub trader_amount_sats: Amount,
    pub timestamp: OffsetDateTime,
    pub txid: Txid,
    pub vout: u32,
}

impl std::fmt::Debug for NewPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewPosition")
            .field("contract_symbol", &self.contract_symbol)
            .field("trader_leverage", &self.trader_leverage)
            .field("quantity", &self.quantity)
            .field("trader_direction", &self.trader_direction)
            // Otherwise we end up printing the hex of the internal representation.
            .field("trader", &self.trader.to_string())
            .field("average_entry_price", &self.average_entry_price)
            .field("trader_liquidation_price", &self.trader_liquidation_price)
            .field(
                "coordinator_liquidation_price",
                &self.coordinator_liquidation_price,
            )
            .field("coordinator_margin", &self.coordinator_margin)
            .field("expiry_timestamp", &self.expiry_timestamp)
            .field("temporary_contract_id", &self.temporary_contract_id)
            .field("coordinator_leverage", &self.coordinator_leverage)
            .field("trader_margin", &self.trader_margin)
            .field("stable", &self.stable)
            .finish()
    }
}

impl std::fmt::Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            trader,
            contract_symbol,
            quantity,
            trader_direction,
            average_entry_price,
            closing_price,
            trader_realized_pnl_sat,
            coordinator_liquidation_price,
            trader_liquidation_price,
            trader_margin,
            coordinator_margin,
            trader_leverage,
            coordinator_leverage,
            position_state,
            order_matching_fees,
            creation_timestamp,
            expiry_timestamp,
            update_timestamp,
            temporary_contract_id,
            stable,
        } = self;

        f.debug_struct("Position")
            .field("id", &id)
            .field("contract_symbol", &contract_symbol)
            .field("trader_leverage", &trader_leverage)
            .field("quantity", &quantity)
            .field("trader_direction", &trader_direction)
            .field("average_entry_price", &average_entry_price)
            .field("trader_liquidation_price", &trader_liquidation_price)
            .field(
                "coordinator_liquidation_price",
                &coordinator_liquidation_price,
            )
            .field("position_state", &position_state)
            .field("coordinator_margin", &coordinator_margin)
            .field("creation_timestamp", &creation_timestamp)
            .field("expiry_timestamp", &expiry_timestamp)
            .field("update_timestamp", &update_timestamp)
            // Otherwise we end up printing the hex of the internal representation.
            .field("trader", &trader.to_string())
            .field("coordinator_leverage", &coordinator_leverage)
            .field("temporary_contract_id", &temporary_contract_id)
            .field("closing_price", &closing_price)
            .field("trader_margin", &trader_margin)
            .field("stable", &stable)
            .field("trader_realized_pnl_sat", &trader_realized_pnl_sat)
            .field("order_matching_fees", &order_matching_fees)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn position_calculate_coordinator_settlement_amount() {
        let position = Position {
            id: 0,
            contract_symbol: ContractSymbol::BtcUsd,
            trader_leverage: 2.0,
            quantity: 100.0,
            trader_direction: Direction::Long,
            average_entry_price: 40_000.0,
            trader_liquidation_price: 20_000.0,
            coordinator_liquidation_price: 60_000.0,
            position_state: PositionState::Open,
            coordinator_margin: 125_000,
            creation_timestamp: OffsetDateTime::now_utc(),
            expiry_timestamp: OffsetDateTime::now_utc(),
            update_timestamp: OffsetDateTime::now_utc(),
            trader: PublicKey::from_str(
                "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
            )
            .unwrap(),
            coordinator_leverage: 2.0,
            temporary_contract_id: None,
            closing_price: None,
            trader_margin: 125_000,
            stable: false,
            trader_realized_pnl_sat: None,
            order_matching_fees: Amount::ZERO,
        };

        let coordinator_settlement_amount = position
            .calculate_coordinator_settlement_amount(dec!(39_000), Amount::from_sat(769))
            .unwrap();

        assert_eq!(coordinator_settlement_amount, 132_179);
    }

    #[test]
    fn position_calculate_coordinator_settlement_amount_trader_leverage_3() {
        let position = Position {
            id: 0,
            contract_symbol: ContractSymbol::BtcUsd,
            trader_leverage: 3.0,
            quantity: 100.0,
            trader_direction: Direction::Long,
            average_entry_price: 40_000.0,
            trader_liquidation_price: 20_000.0,
            coordinator_liquidation_price: 60_000.0,
            position_state: PositionState::Open,
            coordinator_margin: 125_000,
            creation_timestamp: OffsetDateTime::now_utc(),
            expiry_timestamp: OffsetDateTime::now_utc(),
            update_timestamp: OffsetDateTime::now_utc(),
            trader: PublicKey::from_str(
                "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
            )
            .unwrap(),
            coordinator_leverage: 2.0,
            temporary_contract_id: None,
            closing_price: None,
            trader_margin: 125_000,
            stable: false,
            trader_realized_pnl_sat: None,
            order_matching_fees: Amount::ZERO,
        };

        let coordinator_settlement_amount = position
            .calculate_coordinator_settlement_amount(dec!(39_000), Amount::from_sat(769))
            .unwrap();

        assert_eq!(coordinator_settlement_amount, 132_179);
    }

    #[test]
    fn position_calculate_coordinator_settlement_amount_coordinator_leverage_3() {
        let position = Position {
            id: 0,
            contract_symbol: ContractSymbol::BtcUsd,
            trader_leverage: 2.0,
            quantity: 100.0,
            trader_direction: Direction::Long,
            average_entry_price: 40_000.0,
            trader_liquidation_price: 20_000.0,
            coordinator_liquidation_price: 60_000.0,
            position_state: PositionState::Open,
            coordinator_margin: 125_000,
            creation_timestamp: OffsetDateTime::now_utc(),
            expiry_timestamp: OffsetDateTime::now_utc(),
            update_timestamp: OffsetDateTime::now_utc(),
            trader: PublicKey::from_str(
                "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
            )
            .unwrap(),
            coordinator_leverage: 3.0,
            temporary_contract_id: None,
            closing_price: None,
            trader_margin: 125_000,
            stable: false,
            trader_realized_pnl_sat: None,
            order_matching_fees: Amount::ZERO,
        };

        let coordinator_settlement_amount = position
            .calculate_coordinator_settlement_amount(dec!(39_000), Amount::from_sat(769))
            .unwrap();

        assert_eq!(coordinator_settlement_amount, 90_512);
    }

    // Basic sanity tests. Verify the effect of the price moving on the computed settlement amount.

    #[test]
    fn given_long_coordinator_and_price_goes_up() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 1.0;

        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            leverage_coordinator,
            1.0,
            Direction::Long,
            Amount::from_sat(1000),
        )
        .unwrap();

        assert!(margin_coordinator < settlement_coordinator);
    }

    #[test]
    fn given_short_coordinator_and_price_goes_up() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 1.0;

        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            leverage_coordinator,
            Direction::Short,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(settlement_coordinator < margin_coordinator);
    }

    #[test]
    fn given_long_coordinator_and_price_goes_down() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 1.0;

        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            leverage_coordinator,
            1.0,
            Direction::Long,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(settlement_coordinator < margin_coordinator);
    }

    #[test]
    fn given_short_coordinator_and_price_goes_down() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 1.0;

        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            leverage_coordinator,
            Direction::Short,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(margin_coordinator < settlement_coordinator);
    }

    #[test]
    fn given_long_coordinator_and_price_goes_up_different_leverages() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 1.0;

        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            leverage_coordinator,
            2.0,
            Direction::Long,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(margin_coordinator < settlement_coordinator);
    }

    #[test]
    fn given_short_coordinator_and_price_goes_up_different_leverages() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 1.0;

        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            2.0,
            leverage_coordinator,
            Direction::Short,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(settlement_coordinator < margin_coordinator);
    }

    #[test]
    fn given_long_coordinator_and_price_goes_down_different_leverages() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 2.0;

        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            leverage_coordinator,
            1.0,
            Direction::Long,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(settlement_coordinator < margin_coordinator);
    }

    #[test]
    fn given_short_coordinator_and_price_goes_down_different_leverages() {
        let quantity: f32 = 1.0;

        let leverage_coordinator = 2.0;

        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);

        let margin_coordinator = calculate_margin(opening_price, quantity, leverage_coordinator);

        let settlement_coordinator = calculate_coordinator_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            leverage_coordinator,
            Direction::Short,
            Amount::from_sat(13),
        )
        .unwrap();

        assert!(margin_coordinator < settlement_coordinator);
    }

    #[test]
    fn given_trader_long_position_when_no_bid_price_change_then_zero_coordinator_pnl() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(1.0)
            .with_average_entry_price(1000.0)
            .with_direction(Direction::Long);

        let quote = dummy_quote(1000, 0);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, 0);
    }

    #[test]
    fn given_trader_short_position_when_no_ask_price_change_then_zero_coordinator_pnl() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(1.0)
            .with_average_entry_price(1000.0)
            .with_direction(Direction::Short);

        let quote = dummy_quote(0, 1000);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, 0);
    }

    /// Thought Process documentation
    ///
    /// In this example, the trader who went long, bought $20,000 worth of BTC at the price of
    /// 20,000, i.e. 1 BTC At the price of $22,000 the trader sells $20,000 worth of BTC, i.e.
    /// the trader sells it for 0.909090909 BTC. The difference is the trader's profit profit,
    /// i.e.:
    ///
    /// 1 BTC - 0.909090909 BTC = 0.09090909 BTC = 9_090_909 sats profit
    ///
    /// The trader's profit is the coordinator's loss, i.e. -9_090_909.
    /// Note that for the trader the pnl% is +18% because the trader used leverage 2.
    /// For the coordinator the pnl% is -9% because the coordinator used leverage 1.
    ///
    /// See also: `given_long_position_when_price_10_pc_up_then_18pc_profit` test in `trade::cfd`
    #[test]
    fn given_trader_long_position_when_bid_price_10pc_up_then_coordinator_9pc_loss() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(20000.0)
            .with_average_entry_price(20000.0)
            .with_direction(Direction::Long);

        let quote = dummy_quote(22000, 0);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, -9_090_909);
    }

    /// See also: `given_short_position_when_price_10_pc_up_then_18pc_loss` test in `trade::cfd`
    #[test]
    fn given_trader_short_position_when_ask_price_10pc_up_then_coordinator_9pc_profit() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(20000.0)
            .with_average_entry_price(20000.0)
            .with_direction(Direction::Short);

        let quote = dummy_quote(0, 22000);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, 9_090_909);
    }

    /// See also: `given_long_position_when_price_10_pc_down_then_22pc_loss` test in `trade::cfd`
    #[test]
    fn given_trader_long_position_when_bid_price_10pc_down_then_coordinator_11pc_profit() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(20000.0)
            .with_average_entry_price(20000.0)
            .with_direction(Direction::Long);

        let quote = dummy_quote(18000, 0);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, 11_111_111);
    }

    /// See also: `given_short_position_when_price_10_pc_down_then_22pc_profit` test in `trade::cfd`
    #[test]
    fn given_trader_short_position_when_ask_price_10pc_down_then_coordinator_11pc_loss() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(20000.0)
            .with_average_entry_price(20000.0)
            .with_direction(Direction::Short);

        let quote = dummy_quote(0, 18000);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, -11_111_111);
    }

    #[test]
    fn accept_settlement_amount_partial_close_position_reduced() {
        let amount = calculate_accept_settlement_amount_partial_close(
            10_000.0,
            Direction::Long,
            30_000.0,
            2.0,
            2.0,
            5_000.0,
            Direction::Short,
            dec!(20_000),
        )
        .unwrap();

        assert_eq!(amount.to_sat(), 8_333_334);
    }

    #[test]
    fn accept_settlement_amount_partial_close_position_direction_changed() {
        let amount = calculate_accept_settlement_amount_partial_close(
            10_000.0,
            Direction::Long,
            5_000.0,
            2.0,
            2.0,
            15_000.0,
            Direction::Short,
            dec!(6_000),
        )
        .unwrap();

        assert_eq!(amount.to_sat(), 133_333_333);

        let amount = calculate_accept_settlement_amount_partial_close(
            10_000.0,
            Direction::Long,
            5_000.0,
            2.0,
            2.0,
            20_000.0,
            Direction::Short,
            dec!(6_000),
        )
        .unwrap();

        assert_eq!(amount.to_sat(), 133_333_333);
    }

    #[test]
    fn accept_settlement_amount_partial_close_position_increased() {
        let amount = calculate_accept_settlement_amount_partial_close(
            10_000.0,
            Direction::Long,
            5_000.0,
            2.0,
            2.0,
            2_000.0,
            Direction::Long,
            dec!(2_000),
        )
        .unwrap();

        assert_eq!(amount.to_btc(), 1.0);
    }

    #[test]
    #[should_panic]
    fn accept_settlement_amount_partial_close_position_goes_to_zero_panics() {
        let _ = calculate_accept_settlement_amount_partial_close(
            10_000.0,
            Direction::Long,
            5_000.0,
            2.0,
            2.0,
            10_000.0,
            Direction::Short,
            dec!(2_000),
        );
    }

    #[test]
    #[should_panic]
    fn accept_settlement_amount_partial_close_position_unchanged_panics() {
        let _ = calculate_accept_settlement_amount_partial_close(
            10_000.0,
            Direction::Long,
            5_000.0,
            2.0,
            2.0,
            0.0,
            Direction::Short,
            dec!(2_000),
        );
    }

    fn dummy_quote(bid: u64, ask: u64) -> Quote {
        Quote {
            bid_size: 0,
            ask_size: 0,
            bid_price: Decimal::from(bid),
            ask_price: Decimal::from(ask),
            symbol: "".to_string(),
            timestamp: OffsetDateTime::now_utc(),
        }
    }

    impl Position {
        fn dummy() -> Self {
            Position {
                id: 0,
                contract_symbol: ContractSymbol::BtcUsd,
                trader_leverage: 2.0,
                quantity: 100.0,
                trader_direction: Direction::Long,
                average_entry_price: 10000.0,
                trader_liquidation_price: 0.0,
                coordinator_liquidation_price: 0.0,
                position_state: PositionState::Open,
                coordinator_margin: 1000,
                creation_timestamp: OffsetDateTime::now_utc(),
                expiry_timestamp: OffsetDateTime::now_utc(),
                update_timestamp: OffsetDateTime::now_utc(),
                trader: PublicKey::from_str(
                    "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
                )
                .unwrap(),
                temporary_contract_id: None,
                closing_price: None,
                coordinator_leverage: 2.0,
                trader_margin: 1000,
                stable: false,
                trader_realized_pnl_sat: None,
                order_matching_fees: Amount::ZERO,
            }
        }

        fn with_quantity(mut self, quantity: f32) -> Self {
            self.quantity = quantity;
            self
        }

        fn with_average_entry_price(mut self, average_entry_price: f32) -> Self {
            self.average_entry_price = average_entry_price;
            self
        }

        fn with_leverage(mut self, leverage: f32) -> Self {
            self.trader_leverage = leverage;
            self
        }

        fn with_direction(mut self, direction: Direction) -> Self {
            self.trader_direction = direction;
            self
        }
    }
}
