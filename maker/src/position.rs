use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Add;
use uuid::Uuid;

mod tentenone;

pub struct Manager {
    position: Position,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            position: Position::new(),
        }
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PositionUpdateTenTenOne(pub Vec<OrderTenTenOne>);

#[derive(Debug)]
pub struct OrderTenTenOne {
    order_id: Uuid,
    contract_symbol: ContractSymbol,
    /// The number of contracts corresponding to this position update.
    ///
    /// The sign determines the direction: positive is long; negative is short.
    contracts: Decimal,
}

impl OrderTenTenOne {
    pub fn new(order_id: Uuid, contract_symbol: ContractSymbol, contracts: Decimal) -> Self {
        Self {
            order_id,
            contract_symbol,
            contracts,
        }
    }
}

pub struct GetPosition;

pub struct GetPositionResponse {
    pub tentenone: HashMap<ContractSymbol, Decimal>,
}

#[async_trait]
impl xtra::Actor for Manager {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl xtra::Handler<PositionUpdateTenTenOne> for Manager {
    type Return = ();

    async fn handle(
        &mut self,
        update: PositionUpdateTenTenOne,
        _: &mut xtra::Context<Self>,
    ) -> Self::Return {
        for OrderTenTenOne {
            order_id,
            contract_symbol,
            contracts,
        } in update.0
        {
            self.position
                .update_tentenone(contract_symbol, order_id, contracts)
        }
    }
}

#[async_trait]
impl xtra::Handler<GetPosition> for Manager {
    type Return = GetPositionResponse;

    async fn handle(&mut self, _: GetPosition, _: &mut xtra::Context<Self>) -> Self::Return {
        let tentenone =
            HashMap::from_iter(self.position.tentenone.clone().into_iter().map(|position| {
                (
                    position.contract_symbol(),
                    position.contracts().to_decimal(),
                )
            }));

        GetPositionResponse { tentenone }
    }
}

/// The overall position of the maker.
#[derive(Debug)]
struct Position {
    tentenone: HashSet<tentenone::Position>,
}

impl Position {
    pub fn new() -> Self {
        Self {
            tentenone: HashSet::from_iter([tentenone::Position::new(ContractSymbol::BtcUsd)]),
        }
    }

    fn update_tentenone(
        &mut self,
        contract_symbol: ContractSymbol,
        order_id: Uuid,
        contracts: Decimal,
    ) {
        let mut position = self
            .tentenone
            .get(&contract_symbol)
            .cloned()
            .unwrap_or(tentenone::Position::new(ContractSymbol::BtcUsd));

        position.update(order_id, contracts);

        self.tentenone.replace(position);
    }

    #[cfg(test)]
    fn get_tentenone(&self, contract_symbol: &ContractSymbol) -> Decimal {
        match self.tentenone.get(contract_symbol) {
            Some(position) => match position.contracts() {
                Contracts::Balanced => Decimal::ZERO,
                Contracts::Long(contracts) => contracts,
                Contracts::Short(contracts) => -contracts,
            },
            None => Decimal::ZERO,
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Serialize)]
pub enum ContractSymbol {
    BtcUsd,
}

/// The number of contracts in the position, including their direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Contracts {
    /// Either zero contracts or the same number of long and short contracts.
    Balanced,
    /// More long contracts than short.
    Long(Decimal),
    /// More short contracts than long.
    Short(Decimal),
}

impl Contracts {
    fn new(contracts: Decimal) -> Self {
        if contracts.is_zero() {
            Self::Balanced
        } else if contracts.is_sign_positive() {
            Self::Long(contracts)
        } else {
            Self::Short(-contracts)
        }
    }

    fn to_decimal(self) -> Decimal {
        match self {
            Contracts::Balanced => Decimal::ZERO,
            Contracts::Long(contracts) => contracts,
            Contracts::Short(contracts) => -contracts,
        }
    }
}

impl Add for Contracts {
    type Output = Contracts;

    fn add(self, rhs: Self) -> Self::Output {
        let lhs = self.to_decimal();
        let rhs = rhs.to_decimal();

        Self::Output::new(lhs + rhs)
    }
}

impl Display for Contracts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Contracts::Balanced => "Balanced".to_string(),
            Contracts::Long(contracts) => format!("Long {contracts}"),
            Contracts::Short(contracts) => format!("Short {contracts}"),
        };

        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced() {
        let mut position = Position::new();
        let order_id = Uuid::new_v4();
        let contract_symbol = ContractSymbol::BtcUsd;
        let contracts = Decimal::ZERO;

        position.update_tentenone(contract_symbol, order_id, contracts);

        assert_eq!(position.get_tentenone(&contract_symbol), contracts);
    }

    #[test]
    fn long_100() {
        let mut position = Position::new();
        let order_id = Uuid::new_v4();
        let contract_symbol = ContractSymbol::BtcUsd;
        let contracts = Decimal::ONE_HUNDRED;

        position.update_tentenone(contract_symbol, order_id, contracts);

        assert_eq!(position.get_tentenone(&contract_symbol), contracts);
    }

    #[test]
    fn short_100() {
        let mut position = Position::new();
        let order_id = Uuid::new_v4();
        let contract_symbol = ContractSymbol::BtcUsd;
        let contracts = -Decimal::ONE_HUNDRED;

        position.update_tentenone(contract_symbol, order_id, contracts);

        assert_eq!(position.get_tentenone(&contract_symbol), contracts);
    }
}
