use crate::node::COORDINATOR_LEVERAGE;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::ContractId;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::bitmex_client::Quote;
use trade::cfd::calculate_pnl;
use trade::ContractSymbol;
use trade::Direction;

#[derive(Debug, Clone)]
pub struct NewPosition {
    pub contract_symbol: ContractSymbol,
    pub leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub trader: PublicKey,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub collateral: i64,
    pub expiry_timestamp: OffsetDateTime,
    pub temporary_contract_id: ContractId,
}

#[derive(PartialEq, Debug)]
pub enum PositionState {
    Open,
    Closing,
    Closed { pnl: i64 },
}

#[derive(Debug)]
pub struct Position {
    pub id: i32,
    pub contract_symbol: ContractSymbol,
    pub leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub collateral: i64,
    pub creation_timestamp: OffsetDateTime,
    pub expiry_timestamp: OffsetDateTime,
    pub update_timestamp: OffsetDateTime,
    pub trader: PublicKey,

    /// This is the temporary contract id that is created when the contract is being offered
    ///
    /// We use the temporary contract id because the actual contract id might not be known at that
    /// point. The temporary contract id is propagated to all states until the contract is
    /// closed.
    /// This field is optional for backwards compatibility because we cannot deterministically
    /// associate already existing contracts with positions.
    pub temporary_contract_id: Option<ContractId>,
    pub closing_price: Option<f32>,
}

impl Position {
    /// Calculates the profit and loss for the coordinator in satoshis
    ///
    /// The position stored represents the values of the trader.
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
            Direction::Long => (self.leverage, COORDINATOR_LEVERAGE),
            Direction::Short => (COORDINATOR_LEVERAGE, self.leverage),
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
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn given_trader_long_position_when_no_bid_price_change_then_zero_pnl() {
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
    fn given_trader_short_position_when_no_ask_price_change_then_zero_pnl() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(1.0)
            .with_average_entry_price(1000.0)
            .with_direction(Direction::Short);

        let quote = dummy_quote(0, 1000);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, 0);
    }

    /// See also: `given_long_position_when_price_10_pc_up_then_18pc_profit` test in `trade::cfd`
    #[test]
    fn given_trader_long_position_when_bid_price_10pc_up_then_coordinator_18pc_loss() {
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
    fn given_trader_short_position_when_bid_price_10pc_up_then_coordinator_18pc_profit() {
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
    fn given_trader_long_position_when_bid_price_10pc_down_then_coordinator_22pc_profit() {
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
    fn given_trader_short_position_when_bid_price_10pc_down_then_coordinator_22pc_loss() {
        let position = Position::dummy()
            .with_leverage(2.0)
            .with_quantity(20000.0)
            .with_average_entry_price(20000.0)
            .with_direction(Direction::Short);

        let quote = dummy_quote(0, 18000);

        let coordinator_pnl = position.calculate_coordinator_pnl(quote).unwrap();

        assert_eq!(coordinator_pnl, -11_111_111);
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
                leverage: 2.0,
                quantity: 100.0,
                direction: Direction::Long,
                average_entry_price: 10000.0,
                liquidation_price: 0.0,
                position_state: PositionState::Open,
                collateral: 1000,
                creation_timestamp: OffsetDateTime::now_utc(),
                expiry_timestamp: OffsetDateTime::now_utc(),
                update_timestamp: OffsetDateTime::now_utc(),
                trader: PublicKey::from_str(
                    "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
                )
                .unwrap(),
                temporary_contract_id: None,
                closing_price: None,
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
            self.leverage = leverage;
            self
        }

        fn with_direction(mut self, direction: Direction) -> Self {
            self.direction = direction;
            self
        }
    }
}
