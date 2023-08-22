use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::hash::Hash;

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

pub struct PositionUpdateTenTenOne {
    contract_symbol: ContractSymbol,
    /// The number of contracts corresponding to this position update.
    ///
    /// The sign determines the direction: positive is long; negative is short.
    contracts: f32,
}

impl PositionUpdateTenTenOne {
    pub fn new(contract_symbol: ContractSymbol, contracts: f32) -> Self {
        Self {
            contract_symbol,
            contracts,
        }
    }
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
        let PositionUpdateTenTenOne {
            contract_symbol,
            contracts,
        } = update;

        self.position.update_tentenone(contract_symbol, contracts)
    }
}

/// The overall position of the maker.
struct Position {
    tentenone: HashSet<tentenone::Position>,
}

impl Position {
    pub fn new() -> Self {
        Self {
            tentenone: HashSet::from_iter([tentenone::Position::new(ContractSymbol::BtcUsd)]),
        }
    }

    fn update_tentenone(&mut self, contract_symbol: ContractSymbol, contracts: f32) {
        let position = match self.tentenone.get(&contract_symbol) {
            Some(position) => *position,
            None => tentenone::Position::new(contract_symbol),
        };

        let contracts = Decimal::from_f32_retain(contracts).expect("f32 to convert to Decimal");
        self.tentenone.insert(position.update(contracts));
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum ContractSymbol {
    BtcUsd,
}

/// The number of contracts in the position, including their direction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Contracts {
    /// Either zero contracts or the same number of long and short contracts.
    Balanced,
    /// More long contracts than short.
    Long(Decimal),
    /// More short contracts than long.
    Short(Decimal),
}

impl Contracts {
    fn update(self, new_contracts: impl Into<Decimal>) -> Self {
        let new_contracts: Decimal = new_contracts.into();

        if new_contracts.is_zero() {
            self
        } else if new_contracts.is_sign_positive() {
            self.long(new_contracts)
        } else {
            self.short(-new_contracts)
        }
    }

    fn long(self, new_long: Decimal) -> Self {
        debug_assert!(new_long.is_sign_positive());

        match self {
            Self::Balanced => Self::Long(new_long),
            Self::Long(old_long) => Self::Long(old_long + new_long),
            Self::Short(old_short) => {
                let diff = new_long - old_short;

                if diff.is_zero() {
                    Self::Balanced
                } else if diff.is_sign_positive() {
                    Self::Long(diff)
                } else {
                    Self::Short(-diff)
                }
            }
        }
    }

    fn short(self, new_short: Decimal) -> Self {
        debug_assert!(new_short.is_sign_positive());

        match self {
            Self::Balanced => Self::Short(new_short),
            Self::Short(old_short) => Self::Long(old_short + new_short),
            Self::Long(old_long) => {
                let diff = new_short - old_long;

                if diff.is_zero() {
                    Self::Balanced
                } else if diff.is_sign_positive() {
                    Self::Short(diff)
                } else {
                    Self::Long(-diff)
                }
            }
        }
    }
}
