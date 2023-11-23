use crate::compute_relative_contracts;
use crate::decimal_from_f32;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Txid;
use coordinator_commons::TradeParams;
use dlc_manager::ContractId;
use orderbook_commons::order_matching_fee_taker;
use rust_decimal::prelude::Signed;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::bitmex_client::Quote;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_pnl;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Debug, Clone)]
pub struct NewPosition {
    pub contract_symbol: ContractSymbol,
    pub trader_leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub trader: PublicKey,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub coordinator_margin: i64,
    pub expiry_timestamp: OffsetDateTime,
    pub temporary_contract_id: ContractId,
    pub coordinator_leverage: f32,
    pub trader_margin: i64,
    pub stable: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PositionState {
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
    Rollover,
    Resizing,
}

/// The position acts as an aggregate of one contract of one user.
/// The position represents the values of the trader; i.e. the leverage, collateral and direction
/// and the coordinator leverage
#[derive(Clone, Debug)]
pub struct Position {
    pub id: i32,
    pub contract_symbol: ContractSymbol,
    /// the traders leverage
    pub trader_leverage: f32,
    pub quantity: f32,
    /// the traders direction
    pub direction: Direction,
    pub average_entry_price: f32,
    /// the traders liquidation price
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub coordinator_margin: i64,
    pub creation_timestamp: OffsetDateTime,
    pub expiry_timestamp: OffsetDateTime,
    pub update_timestamp: OffsetDateTime,
    pub trader: PublicKey,
    /// the coordinators leverage
    pub coordinator_leverage: f32,

    /// The temporary contract id that is created when the contract is being offered
    ///
    /// We use the temporary contract id because the actual contract id might not be known at that
    /// point. The temporary contract id is propagated to all states until the contract is
    /// closed.
    /// This field is optional for backwards compatibility because we cannot deterministically
    /// associate already existing contracts with positions.
    pub temporary_contract_id: Option<ContractId>,
    pub closing_price: Option<f32>,
    pub trader_margin: i64,
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
            None => quote.get_price_for_direction(self.direction.opposite()),
            Some(closing_price) => {
                Decimal::try_from(closing_price).expect("f32 closing price to fit into decimal")
            }
        };

        let average_entry_price = Decimal::try_from(self.average_entry_price)
            .context("Failed to convert average entry price to Decimal")?;

        let (long_leverage, short_leverage) = match self.direction {
            Direction::Long => (self.trader_leverage, self.coordinator_leverage),
            Direction::Short => (self.coordinator_leverage, self.trader_leverage),
        };

        // the position in the database is the trader's position, our direction is opposite
        let direction = self.direction.opposite();

        let pnl = calculate_pnl(
            average_entry_price,
            closing_price,
            self.quantity,
            long_leverage,
            short_leverage,
            direction,
        )
        .context("Failed to calculate pnl for position")?;

        Ok(pnl)
    }

    /// Calculate the settlement amount for the accept party (i.e. the trader) when closing the
    /// _entire_ position.
    pub fn calculate_accept_settlement_amount(&self, closing_price: Decimal) -> Result<u64> {
        let opening_price = Decimal::try_from(self.average_entry_price)?;

        let leverage_long = leverage_long(
            self.direction,
            self.trader_leverage,
            self.coordinator_leverage,
        );
        let leverage_short = leverage_short(
            self.direction,
            self.trader_leverage,
            self.coordinator_leverage,
        );

        calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            self.quantity,
            leverage_long,
            leverage_short,
            self.direction,
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
            self.direction,
            self.average_entry_price,
            self.trader_leverage,
            self.coordinator_leverage,
            trade_params.quantity,
            trade_params.direction,
            trade_params.average_execution_price(),
        )
    }
}

/// Calculate the accept settlement amount based on the PNL.
fn calculate_accept_settlement_amount(
    opening_price: Decimal,
    closing_price: Decimal,
    quantity: f32,
    long_leverage: f32,
    short_leverage: f32,
    direction: Direction,
) -> Result<u64> {
    let close_position_fee = order_matching_fee_taker(quantity, closing_price).to_sat() as i64;

    let pnl = calculate_pnl(
        opening_price,
        closing_price,
        quantity,
        long_leverage,
        short_leverage,
        direction,
    )?;

    let leverage = match direction {
        Direction::Long => long_leverage,
        Direction::Short => short_leverage,
    };

    let margin_trader_without_opening_fees = calculate_margin(opening_price, quantity, leverage);

    let accept_settlement_amount = Decimal::from(margin_trader_without_opening_fees)
        + Decimal::from(pnl)
        - Decimal::from(close_position_fee);
    // the amount can only be positive, adding a safeguard here with the max comparison to
    // ensure the i64 fits into u64
    let accept_settlement_amount = accept_settlement_amount
        .max(Decimal::ZERO)
        .to_u64()
        .expect("to fit into u64");
    Ok(accept_settlement_amount)
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

        let pnl = calculate_pnl(
            decimal_from_f32(position_average_execution_price),
            trade_average_execution_price,
            settled_contracts,
            leverage_long,
            leverage_short,
            position_direction,
        )?;

        ((position_trader_margin as i64) + pnl).max(0) as u64
    }
    // Position changed direction.
    else if contracts_before_relative.signum() != contracts_after_relative.signum()
        && !contracts_after_relative.is_zero()
    {
        // Settled as many contracts as there are in the entire position.
        let settled_contracts = position_quantity;

        let pnl = calculate_pnl(
            decimal_from_f32(position_average_execution_price),
            trade_average_execution_price,
            settled_contracts,
            leverage_long,
            leverage_short,
            position_direction,
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    // some basic sanity tests, that in case the position goes the right or wrong way the settlement
    // amount is moving correspondingly up or down.

    #[test]
    fn given_a_long_position_and_a_larger_closing_price() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount > margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_larger_closing_price() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_long_position_and_a_smaller_closing_price() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_smaller_closing_price() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount > margin_trader);
    }

    #[test]
    fn given_a_long_position_and_a_larger_closing_price_different_leverages() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            2.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 2.0);
        assert!(accept_settlement_amount > margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_larger_closing_price_different_leverages() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            2.0,
            1.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_long_position_and_a_smaller_closing_price_different_leverages() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            2.0,
            1.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 2.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_smaller_closing_price_different_leverages() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            2.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 2.0);
        assert!(accept_settlement_amount > margin_trader);
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
        pub(crate) fn dummy() -> Self {
            Position {
                id: 0,
                contract_symbol: ContractSymbol::BtcUsd,
                trader_leverage: 2.0,
                quantity: 100.0,
                direction: Direction::Long,
                average_entry_price: 10000.0,
                liquidation_price: 0.0,
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
            }
        }

        pub(crate) fn with_quantity(mut self, quantity: f32) -> Self {
            self.quantity = quantity;
            self
        }

        pub(crate) fn with_average_entry_price(mut self, average_entry_price: f32) -> Self {
            self.average_entry_price = average_entry_price;
            self
        }

        pub(crate) fn with_leverage(mut self, leverage: f32) -> Self {
            self.trader_leverage = leverage;
            self
        }

        pub(crate) fn with_direction(mut self, direction: Direction) -> Self {
            self.direction = direction;
            self
        }
    }
}

#[derive(Clone, Debug)]
pub struct CollaborativeRevert {
    pub channel_id: [u8; 32],
    pub trader_pubkey: PublicKey,
    pub price: f32,
    pub coordinator_address: Address,
    pub coordinator_amount_sats: Amount,
    pub trader_amount_sats: Amount,
    pub timestamp: OffsetDateTime,
    pub txid: Txid,
    pub vout: u32,
}
