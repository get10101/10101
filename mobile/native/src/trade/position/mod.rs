use crate::calculations::calculate_liquidation_price;
use crate::calculations::calculate_pnl;
use crate::trade::order::Order;
use crate::trade::order::OrderState;
use crate::trade::order::OrderType;
use crate::trade::Trade;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::Amount;
use bitcoin::SignedAmount;
use commons::order_matching_fee_taker;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use serde::Serialize;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;

pub mod api;
pub mod handler;

#[derive(Debug, Clone, PartialEq, Copy, Serialize)]
pub enum PositionState {
    /// The position is open
    ///
    /// Open in the sense, that there is an active position that is being rolled-over.
    /// Note that a "closed" position does not exist, but is just removed.
    /// During the process of getting closed (after creating the counter-order that will wipe out
    /// the position), the position is in state "Closing".
    ///
    /// Transitions:
    /// ->Open
    /// Rollover->Open
    Open,
    /// The position is in the process of being closed
    ///
    /// The user has created an order that will wipe out the position.
    /// Once this order has been filled the "closed" the position is not shown in the user
    /// interface, so we don't have a "closed" state because no position data will be provided to
    /// the user interface.
    /// Transitions:
    /// Open->Closing
    Closing,
    /// The position is in rollover
    ///
    /// This is a technical intermediate state indicating that a rollover is currently in progress.
    ///
    /// Transitions:
    /// Open->Rollover
    Rollover,
    /// The position is being resized.
    ///
    /// Transitions:
    /// Open->Resizing.
    Resizing,
}

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub leverage: f32,
    pub quantity: f32,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub collateral: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub expiry: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    pub stable: bool,
}

impl Position {
    /// Construct a new open position from an initial [`OrderState::Filled`] order.
    pub fn new_open(
        order: Order,
        actual_collateral_sat: u64,
        expiry: OffsetDateTime,
    ) -> (Self, Trade) {
        let now_timestamp = OffsetDateTime::now_utc();

        let average_entry_price = order.execution_price().expect("order to be filled");

        let liquidation_price =
            calculate_liquidation_price(average_entry_price, order.leverage, order.direction);

        let contracts = decimal_from_f32(order.quantity);

        // we care about checking the margin only if it is not a margin order. If it is a margin
        // order, then the margin has been defined in the order
        if OrderType::Margin != order.order_type {
            let leverage = decimal_from_f32(order.leverage);
            let average_entry_price = decimal_from_f32(average_entry_price);

            let expected_collateral_btc = contracts / (leverage * average_entry_price);
            let expected_collateral_btc = expected_collateral_btc
                .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                .to_f64()
                .expect("collateral to fit into f64");

            let calculated_collateral_sat = Amount::from_btc(expected_collateral_btc)
                .expect("collateral to fit into Amount")
                .to_sat();

            debug_assert!(
                actual_collateral_sat == calculated_collateral_sat,
                "actual_collateral_sat = {}, calculated_collateral_sat = {}",
                actual_collateral_sat,
                calculated_collateral_sat
            );

            if actual_collateral_sat != calculated_collateral_sat {
                tracing::debug!(
                    actual_sat = %actual_collateral_sat,
                    expected_sat = %calculated_collateral_sat,
                    "Actual DLC collateral for new position different to calculated"
                );
            }
        }

        let position = Self {
            leverage: order.leverage,
            quantity: order.quantity,
            contract_symbol: order.contract_symbol,
            direction: order.direction,
            average_entry_price,
            liquidation_price,
            position_state: PositionState::Open,
            collateral: actual_collateral_sat,
            expiry,
            updated: now_timestamp,
            created: now_timestamp,
            stable: order.stable,
        };

        let average_entry_price = decimal_from_f32(average_entry_price);
        let fee = order_matching_fee_taker(order.quantity, average_entry_price);

        let margin_diff = SignedAmount::from_sat(actual_collateral_sat as i64);

        let trade_cost = trade_cost(margin_diff, SignedAmount::ZERO, fee);

        let trade = Trade {
            order_id: order.id,
            contract_symbol: order.contract_symbol,
            contracts,
            direction: order.direction,
            trade_cost,
            fee,
            pnl: None,
            price: average_entry_price,
            timestamp: now_timestamp,
        };

        (position, trade)
    }

    pub fn apply_order(
        self,
        order: Order,
        expiry: OffsetDateTime,
        actual_collateral_sat: u64,
    ) -> Result<(Option<Self>, Vec<Trade>)> {
        match order {
            Order {
                state: OrderState::Filled { .. } | OrderState::Filling { .. },
                ..
            } => {}
            _ => bail!("Cannot apply order that is not filling or filled"),
        };

        ensure!(
            self.contract_symbol == order.contract_symbol,
            "Cannot apply order to position if contract symbol does not match"
        );

        ensure!(
            order.order_type == OrderType::Market,
            "Cannot apply limit order to position"
        );

        let mut trades = Vec::new();
        let position = self.apply_order_recursive(order, expiry, &mut trades)?;

        {
            let calculated_collateral_sat =
                position.as_ref().map(|p| p.collateral).unwrap_or_default();

            if actual_collateral_sat != calculated_collateral_sat {
                tracing::debug!(
                    actual_sat = %actual_collateral_sat,
                    calculated_sat = %calculated_collateral_sat,
                    "Actual DLC collateral for position different to calculated"
                );
            }
        }

        Ok((position, trades))
    }

    /// Apply an [`Order`] to the [`Position`] recursively until the order is fully applied. Each
    /// application of the order can (1) reduce the position, (2) reduce the position down to zero
    /// contracts or (3) increase the position. By combining (2) and (3) through recursion we are
    /// able to apply orders which change the direction of the position.
    ///
    /// NOTE: The order's leverage is ignored when applying an order to an existing position. It
    /// does not seem to make much sense to allow a trader to both change the number and/or
    /// direction of contracts (the point of creating a new order) _and_ to change the leverage.
    /// Also, it's not so straightforward to calculate combined leverages, particularly when
    /// reducing a position.
    ///
    /// TODO: This highlights the fact that orders really shouldn't have a leverage associated with
    /// them!
    fn apply_order_recursive(
        self,
        order: Order,
        expiry: OffsetDateTime,
        trades: &mut Vec<Trade>,
    ) -> Result<Option<Self>> {
        // The order has been fully applied.
        if order.quantity == 0.0 {
            // The position has vanished.
            if self.quantity == 0.0 {
                return Ok(None);
            }
            // What is left of the position after fully applying the order.
            else {
                return Ok(Some(self));
            }
        }

        let now_timestamp = OffsetDateTime::now_utc();

        let order_id = order.id;

        let starting_contracts = decimal_from_f32(self.quantity);
        let starting_leverage = decimal_from_f32(self.leverage);
        let starting_average_execution_price = decimal_from_f32(self.average_entry_price);

        let order_contracts = decimal_from_f32(order.quantity);
        let order_execution_price = decimal_from_f32(
            order
                .execution_price()
                .expect("order to have an execution price"),
        );

        // If the directions differ (and the position has contracts!) we must reduce the position
        // (first).
        let (position, order, trade): (Position, Order, Trade) = if self.quantity != 0.0
            && order.direction != self.direction
        {
            let contract_diff = self.quantity - order.quantity;

            // Reduce position and order to 0.
            if contract_diff == 0.0 {
                let fee = order_matching_fee_taker(order.quantity, order_execution_price);

                // The margin difference corresponds to the entire margin for the position being
                // closed, as a negative number.
                let margin_diff = {
                    let margin_before_btc =
                        starting_contracts / (starting_leverage * starting_average_execution_price);

                    let margin_before_btc = margin_before_btc
                        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                        .to_f64()
                        .expect("margin to fit into f64");

                    // `margin_before_btc` is a positive number so we have to make it negative so
                    // that reducing the position results in a negative `trade_cost` i.e. money into
                    // the Lightning wallet.
                    SignedAmount::from_btc(-margin_before_btc)
                        .expect("margin diff to fit into SignedAmount")
                };

                let pnl = {
                    let pnl = calculate_pnl(
                        self.average_entry_price,
                        trade::Price {
                            bid: order_execution_price,
                            ask: order_execution_price,
                        },
                        order.quantity,
                        self.leverage,
                        self.direction,
                    )?;
                    SignedAmount::from_sat(pnl)
                };

                let trade_cost = trade_cost(margin_diff, pnl, fee);

                let trade = Trade {
                    order_id,
                    contract_symbol: order.contract_symbol,
                    contracts: order_contracts,
                    direction: order.direction,
                    trade_cost,
                    fee,
                    pnl: Some(pnl),
                    price: order_execution_price,
                    timestamp: now_timestamp,
                };

                let position = Position {
                    quantity: 0.0,
                    ..self
                };

                let order = Order {
                    quantity: 0.0,
                    ..order
                };

                (position, order, trade)
            }
            // Reduce position and consume entire order.
            else if contract_diff.is_sign_positive() {
                let starting_contracts_relative =
                    compute_relative_contracts(self.quantity, self.direction);
                let order_contracts_relative =
                    compute_relative_contracts(order.quantity, order.direction);
                let total_contracts_relative =
                    starting_contracts_relative + order_contracts_relative;

                // We keep using the starting average execution price because we have only reduced
                // the number of contracts in the position.

                let updated_collateral = {
                    let updated_collateral_btc = total_contracts_relative
                        / (starting_leverage * starting_average_execution_price);

                    let updated_collateral_btc = updated_collateral_btc
                        .abs()
                        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                        .to_f64()
                        .expect("collateral to fit into f64");

                    Amount::from_btc(updated_collateral_btc)
                        .expect("collateral to fit into Amount")
                        .to_sat()
                };

                let position = Position {
                    leverage: f32_from_decimal(starting_leverage),
                    quantity: contract_diff,
                    contract_symbol: self.contract_symbol,
                    direction: self.direction,
                    average_entry_price: self.average_entry_price,
                    liquidation_price: self.liquidation_price,
                    position_state: PositionState::Open,
                    collateral: updated_collateral,
                    expiry,
                    updated: now_timestamp,
                    created: self.created,
                    stable: self.stable,
                };

                let fee = order_matching_fee_taker(order.quantity, order_execution_price);

                let margin_diff = {
                    let margin_before_btc = starting_contracts_relative.abs()
                        / (starting_leverage * starting_average_execution_price);

                    let margin_after_btc = total_contracts_relative.abs()
                        / (starting_leverage * starting_average_execution_price);

                    let margin_diff_btc = (margin_after_btc.abs() - margin_before_btc.abs())
                        .abs()
                        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                        .to_f64()
                        .expect("margin to fit into f64");

                    // `margin_change_btc` is a positive number since we calculated it using `abs`.
                    // Because this is the result of applying an order that reduces the position we
                    // want `trade_cost` to be negative so we add the negative sign.
                    SignedAmount::from_btc(-margin_diff_btc)
                        .expect("margin to fit into SignedAmount")
                };

                let pnl = {
                    let pnl = calculate_pnl(
                        self.average_entry_price,
                        trade::Price {
                            bid: order_execution_price,
                            ask: order_execution_price,
                        },
                        order.quantity,
                        self.leverage,
                        self.direction,
                    )?;
                    SignedAmount::from_sat(pnl)
                };

                let trade_cost = trade_cost(margin_diff, pnl, fee);

                let trade = Trade {
                    order_id,
                    contract_symbol: self.contract_symbol,
                    contracts: order_contracts_relative.abs(),
                    direction: order.direction,
                    trade_cost,
                    fee,
                    pnl: Some(pnl),
                    price: order_execution_price,
                    timestamp: now_timestamp,
                };

                let order = Order {
                    quantity: 0.0,
                    ..order
                };

                (position, order, trade)
            }
            // Reduce position to 0, with leftover order.
            else {
                let leftover_order_contracts = contract_diff.abs();

                // This trade only includes the fee for the part of the other that was applied
                // thus far.
                let fee = order_matching_fee_taker(self.quantity, order_execution_price);

                // The margin difference corresponds to the entire margin for the position being
                // closed, as a negative number.
                let margin_diff = {
                    let margin_before_btc =
                        starting_contracts / (starting_leverage * starting_average_execution_price);

                    let margin_before_btc = margin_before_btc
                        .abs()
                        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                        .to_f64()
                        .expect("margin to fit into f64");

                    // `margin_before_btc` is a positive number so we have to make it negative so
                    // that reducing the position results in a negative `trade_cost` i.e. money into
                    // the Lightning wallet.
                    SignedAmount::from_btc(-margin_before_btc)
                        .expect("margin to fit into SignedAmount")
                };

                let pnl = {
                    let pnl = calculate_pnl(
                        self.average_entry_price,
                        trade::Price {
                            bid: order_execution_price,
                            ask: order_execution_price,
                        },
                        self.quantity,
                        self.leverage,
                        self.direction,
                    )?;
                    SignedAmount::from_sat(pnl)
                };

                let trade_cost = trade_cost(margin_diff, pnl, fee);

                let trade = Trade {
                    order_id,
                    contract_symbol: order.contract_symbol,
                    contracts: starting_contracts,
                    direction: order.direction,
                    trade_cost,
                    fee,
                    pnl: Some(pnl),
                    price: order_execution_price,
                    timestamp: now_timestamp,
                };

                let position = Position {
                    quantity: 0.0,
                    ..self
                };

                // Reduce the order without vanishing it.
                let order = Order {
                    quantity: leftover_order_contracts,
                    ..order
                };

                (position, order, trade)
            }
        }
        // If the directions agree or the position has no contracts we must increase the position.
        else {
            let starting_contracts_relative =
                compute_relative_contracts(self.quantity, self.direction);
            let order_contracts_relative =
                compute_relative_contracts(order.quantity, order.direction);
            let total_contracts_relative = starting_contracts_relative + order_contracts_relative;

            let updated_average_execution_price = total_contracts_relative
                / (starting_contracts_relative / starting_average_execution_price
                    + order_contracts_relative / order_execution_price);

            let updated_liquidation_price = calculate_liquidation_price(
                f32_from_decimal(updated_average_execution_price),
                f32_from_decimal(starting_leverage),
                self.direction,
            );

            let updated_collateral = {
                let updated_collateral_btc = total_contracts_relative
                    / (starting_leverage * updated_average_execution_price);

                let updated_collateral_btc = updated_collateral_btc
                    .abs()
                    .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                    .to_f64()
                    .expect("collateral to fit into f64");

                Amount::from_btc(updated_collateral_btc)
                    .expect("collateral to fit into Amount")
                    .to_sat()
            };

            let stable = self.stable && order.stable && self.direction == Direction::Short;

            let position = Position {
                leverage: f32_from_decimal(starting_leverage),
                quantity: f32_from_decimal(total_contracts_relative.abs()),
                contract_symbol: self.contract_symbol,
                direction: order.direction,
                average_entry_price: f32_from_decimal(updated_average_execution_price),
                liquidation_price: updated_liquidation_price,
                position_state: PositionState::Open,
                collateral: updated_collateral,
                expiry,
                updated: now_timestamp,
                created: self.created,
                stable,
            };

            let fee = order_matching_fee_taker(order.quantity, order_execution_price);

            let margin_diff = {
                let margin_before_btc = starting_contracts_relative.abs()
                    / (starting_leverage * starting_average_execution_price);

                let margin_after_btc = total_contracts_relative.abs()
                    / (starting_leverage * updated_average_execution_price);

                let margin_diff_btc = (margin_after_btc - margin_before_btc)
                    .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
                    .to_f64()
                    .expect("margin to fit into f64");

                SignedAmount::from_btc(margin_diff_btc).expect("margin to fit into SignedAmount")
            };

            let trade_cost = trade_cost(margin_diff, SignedAmount::ZERO, fee);

            let trade = Trade {
                order_id,
                contract_symbol: order.contract_symbol,
                contracts: order_contracts,
                direction: order.direction,
                trade_cost,
                fee,
                pnl: None,
                price: order_execution_price,
                timestamp: now_timestamp,
            };

            let order = Order {
                quantity: 0.0,
                ..order
            };

            (position, order, trade)
        };

        trades.push(trade);

        position.apply_order_recursive(order, expiry, trades)
    }
}

/// The _cost_ of a trade is computed as the change in margin (positive if the margin _increases_),
/// plus the PNL (positive if the PNL is a loss), plus the fee (always positive because fees are
/// always a cost).
fn trade_cost(margin_diff: SignedAmount, pnl: SignedAmount, fee: Amount) -> SignedAmount {
    let fee = fee.to_signed().expect("fee to fit into SignedAmount");

    // We have to flip the sign of the PNL because it inherently uses _negative numbers for losses_,
    // but here we want _costs to be positive_.
    margin_diff - pnl + fee
}

#[track_caller]
fn decimal_from_f32(float: f32) -> Decimal {
    Decimal::from_f32(float).expect("f32 to fit into Decimal")
}

#[track_caller]
fn f32_from_decimal(decimal: Decimal) -> f32 {
    decimal.to_f32().expect("Decimal to fit into f32")
}

/// Compute the number of contracts for the [`Order`] relative to its [`Direction`].
fn compute_relative_contracts(contracts: f32, direction: Direction) -> Decimal {
    let contracts = decimal_from_f32(contracts)
        // We round to 2 decimal places to avoid slight differences between opening and
        // closing orders.
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

    match direction {
        Direction::Long => contracts,
        Direction::Short => -contracts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::order::OrderReason;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[test]
    fn open_position() {
        let now = OffsetDateTime::now_utc();

        let dlc_collateral = 78_125;

        let order = Order {
            id: Uuid::new_v4(),
            leverage: 1.0,
            quantity: 25.0,
            margin_sats: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Short,
            order_type: OrderType::Market,
            state: OrderState::Filled {
                execution_price: 32_000.0,
            },
            creation_timestamp: now,
            order_expiry_timestamp: now,
            reason: OrderReason::Manual,
            stable: true,
            failure_reason: None,
        };

        let (position, opening_trade) = Position::new_open(order.clone(), dlc_collateral, now);

        assert_eq!(position.leverage, 1.0);
        assert_eq!(position.quantity, 25.0);
        assert_eq!(position.contract_symbol, position.contract_symbol);
        assert_eq!(position.direction, order.direction);
        assert_eq!(position.average_entry_price, 32_000.0);
        assert_eq!(position.liquidation_price, 1_048_575.0);
        assert_eq!(position.position_state, PositionState::Open);
        assert_eq!(position.collateral, 78_125);
        assert!(position.stable);

        assert_eq!(opening_trade.order_id, order.id);
        assert_eq!(opening_trade.contract_symbol, order.contract_symbol);
        assert_eq!(opening_trade.contracts, dec!(25));
        assert_eq!(opening_trade.direction, order.direction);
        assert_eq!(opening_trade.trade_cost, SignedAmount::from_sat(78_359));
        assert_eq!(opening_trade.fee, Amount::from_sat(234));
        assert_eq!(opening_trade.pnl, None);
        assert_eq!(
            opening_trade.price,
            decimal_from_f32(order.execution_price().unwrap())
        );
    }

    #[test]
    fn close_position() {
        let now = OffsetDateTime::now_utc();

        let position = Position {
            leverage: 2.0,
            quantity: 10.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Long,
            average_entry_price: 36_469.5,
            liquidation_price: 24_313.0,
            position_state: PositionState::Open,
            collateral: 13_710,
            expiry: now,
            updated: now,
            created: now,
            stable: false,
        };

        let order = Order {
            id: Uuid::new_v4(),
            leverage: 2.0,
            quantity: 10.0,
            margin_sats: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Short,
            order_type: OrderType::Market,
            state: OrderState::Filled {
                execution_price: 36_401.5,
            },
            creation_timestamp: now,
            order_expiry_timestamp: now,
            reason: OrderReason::Manual,
            stable: false,
            failure_reason: None,
        };

        // The DLC channel has been closed.
        let dlc_collateral_after_resize = 0;
        let (updated_position, trades) = position
            .apply_order(order.clone(), now, dlc_collateral_after_resize)
            .unwrap();

        assert!(updated_position.is_none());

        let closing_trade = match trades.as_slice() {
            [closing_trade] => closing_trade,
            trades => panic!("Unexpected number of trades: {}", trades.len()),
        };

        assert_eq!(closing_trade.order_id, order.id);
        assert_eq!(closing_trade.contract_symbol, order.contract_symbol);
        assert_eq!(closing_trade.contracts, Decimal::TEN);
        assert_eq!(closing_trade.direction, order.direction);
        assert_eq!(closing_trade.trade_cost, SignedAmount::from_sat(-13_577));
        assert_eq!(closing_trade.fee, Amount::from_sat(82));
        assert_eq!(closing_trade.pnl, Some(SignedAmount::from_sat(-51)));
        assert_eq!(
            closing_trade.price,
            decimal_from_f32(order.execution_price().unwrap())
        );
    }

    #[test]
    fn extend_position() {
        let now = OffsetDateTime::now_utc();

        let position = Position {
            leverage: 2.0,
            quantity: 10.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Long,
            average_entry_price: 36_469.5,
            liquidation_price: 24_313.0,
            position_state: PositionState::Resizing,
            collateral: 13_710,
            expiry: now,
            updated: now,
            created: now,
            stable: false,
        };

        let order = Order {
            id: Uuid::new_v4(),
            leverage: 2.0,
            quantity: 5.0,
            margin_sats: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Long,
            order_type: OrderType::Market,
            state: OrderState::Filled {
                execution_price: 36_401.5,
            },
            creation_timestamp: now,
            order_expiry_timestamp: now,
            reason: OrderReason::Manual,
            stable: false,
            failure_reason: None,
        };

        let dlc_collateral_after_resize = 20_578;
        let (updated_position, trades) = position
            .clone()
            .apply_order(order.clone(), now, dlc_collateral_after_resize)
            .unwrap();
        let updated_position = updated_position.unwrap();

        assert_eq!(updated_position.leverage, 2.0);
        assert_eq!(updated_position.quantity, 15.0);
        assert_eq!(updated_position.contract_symbol, position.contract_symbol);
        assert_eq!(updated_position.direction, position.direction);
        assert_eq!(updated_position.average_entry_price, 36_446.805);
        assert_eq!(updated_position.liquidation_price, 24297.873);
        assert_eq!(updated_position.position_state, PositionState::Open);
        assert_eq!(updated_position.collateral, 20_578);
        assert!(!updated_position.stable);

        let trade = match trades.as_slice() {
            [trade] => trade,
            trades => panic!("Unexpected number of trades: {}", trades.len()),
        };

        assert_eq!(trade.order_id, order.id);
        assert_eq!(trade.contract_symbol, order.contract_symbol);
        assert_eq!(trade.contracts, dec!(5));
        assert_eq!(trade.direction, order.direction);
        assert_eq!(trade.trade_cost, SignedAmount::from_sat(6_909));
        assert_eq!(trade.fee, Amount::from_sat(41));
        assert_eq!(trade.pnl, None);
        assert_eq!(
            trade.price,
            decimal_from_f32(order.execution_price().unwrap())
        );
    }

    #[test]
    fn reduce_position() {
        let now = OffsetDateTime::now_utc();

        let position = Position {
            leverage: 2.0,
            quantity: 10.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Long,
            average_entry_price: 36_469.5,
            liquidation_price: 24_313.0,
            position_state: PositionState::Resizing,
            collateral: 13_710,
            expiry: now,
            updated: now,
            created: now,
            stable: false,
        };

        let order = Order {
            id: Uuid::new_v4(),
            leverage: 2.0,
            quantity: 5.0,
            margin_sats: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Short,
            order_type: OrderType::Market,
            state: OrderState::Filled {
                execution_price: 36_401.5,
            },
            creation_timestamp: now,
            order_expiry_timestamp: now,
            reason: OrderReason::Manual,
            stable: false,
            failure_reason: None,
        };

        let dlc_collateral_after_resize = 6_855;
        let (updated_position, trades) = position
            .clone()
            .apply_order(order.clone(), now, dlc_collateral_after_resize)
            .unwrap();
        let updated_position = updated_position.unwrap();

        assert_eq!(updated_position.leverage, 2.0);
        assert_eq!(updated_position.quantity, 5.0);
        assert_eq!(updated_position.contract_symbol, position.contract_symbol);
        assert_eq!(updated_position.direction, position.direction);
        assert_eq!(
            updated_position.average_entry_price,
            position.average_entry_price
        );
        assert_eq!(
            updated_position.liquidation_price,
            position.liquidation_price
        );
        assert_eq!(updated_position.position_state, PositionState::Open);
        assert_eq!(updated_position.collateral, 6_855);
        assert!(!updated_position.stable);

        let trade = match trades.as_slice() {
            [trade] => trade,
            trades => panic!("Unexpected number of trades: {}", trades.len()),
        };

        assert_eq!(trade.order_id, order.id);
        assert_eq!(trade.contract_symbol, order.contract_symbol);
        assert_eq!(trade.contracts, dec!(5));
        assert_eq!(trade.direction, order.direction);
        assert_eq!(trade.trade_cost, SignedAmount::from_sat(-6_788));
        assert_eq!(trade.fee, Amount::from_sat(41));
        assert_eq!(trade.pnl, Some(SignedAmount::from_sat(-26)));
        assert_eq!(
            trade.price,
            decimal_from_f32(order.execution_price().unwrap())
        );
    }

    #[test]
    fn resize_position_from_long_to_short() {
        let now = OffsetDateTime::now_utc();

        let position = Position {
            leverage: 2.0,
            quantity: 10.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Long,
            average_entry_price: 36_469.5,
            liquidation_price: 24_313.0,
            position_state: PositionState::Resizing,
            collateral: 13_710,
            expiry: now,
            updated: now,
            created: now,
            stable: false,
        };

        let order = Order {
            id: Uuid::new_v4(),
            leverage: 2.0,
            quantity: 20.0,
            margin_sats: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Short,
            order_type: OrderType::Market,
            state: OrderState::Filled {
                execution_price: 36_401.5,
            },
            creation_timestamp: now,
            order_expiry_timestamp: now,
            reason: OrderReason::Manual,
            stable: false,
            failure_reason: None,
        };

        let dlc_collateral_after_resize = 13_736;
        let (updated_position, trades) = position
            .clone()
            .apply_order(order.clone(), now, dlc_collateral_after_resize)
            .unwrap();
        let updated_position = updated_position.unwrap();

        assert_eq!(updated_position.leverage, 2.0);
        assert_eq!(updated_position.quantity, 10.0);
        assert_eq!(updated_position.contract_symbol, position.contract_symbol);
        assert_eq!(updated_position.direction, order.direction);
        assert_eq!(updated_position.average_entry_price, 36_401.5);
        assert_eq!(updated_position.liquidation_price, 24_267.666);
        assert_eq!(updated_position.position_state, PositionState::Open);
        assert_eq!(updated_position.collateral, 13_736);
        assert!(!updated_position.stable);

        let (closing_trade, opening_trade) = match trades.as_slice() {
            [closing_trade, opening_trade] => (closing_trade, opening_trade),
            trades => panic!("Unexpected number of trades: {}", trades.len()),
        };

        assert_eq!(closing_trade.order_id, order.id);
        assert_eq!(closing_trade.contract_symbol, order.contract_symbol);
        assert_eq!(closing_trade.contracts, Decimal::TEN);
        assert_eq!(closing_trade.direction, order.direction);
        assert_eq!(closing_trade.trade_cost, SignedAmount::from_sat(-13_577));
        assert_eq!(closing_trade.fee, Amount::from_sat(82));
        assert_eq!(closing_trade.pnl, Some(SignedAmount::from_sat(-51)));
        assert_eq!(
            closing_trade.price,
            decimal_from_f32(order.execution_price().unwrap())
        );

        assert_eq!(opening_trade.order_id, order.id);
        assert_eq!(opening_trade.contract_symbol, order.contract_symbol);
        assert_eq!(opening_trade.contracts, Decimal::TEN);
        assert_eq!(opening_trade.direction, order.direction);
        assert_eq!(opening_trade.trade_cost, SignedAmount::from_sat(13_818));
        assert_eq!(opening_trade.fee, Amount::from_sat(82));
        assert_eq!(opening_trade.pnl, None);
        assert_eq!(
            opening_trade.price,
            decimal_from_f32(order.execution_price().unwrap())
        );
    }
}
