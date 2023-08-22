use crate::position::ContractSymbol;
use crate::position::Contracts;
use rust_decimal::Decimal;
use std::borrow::Borrow;
use std::hash::Hash;
use std::hash::Hasher;

/// Simulation of what the maker's position for a particular [`ContractSymbol`] would be if there
/// was a DLC channel between maker and coordinator.
///
/// TODO: Eventually this should be a shadow representation of the position in the maker-coordinator
/// DLC channel.
#[derive(Clone, Copy, Eq)]
pub struct Position {
    contract_symbol: ContractSymbol,
    contracts: Contracts,
}

impl Position {
    pub fn new(contract_symbol: ContractSymbol) -> Self {
        Self {
            contract_symbol,
            contracts: Contracts::Balanced,
        }
    }

    pub fn update(self, contracts: impl Into<Decimal>) -> Self {
        let contracts = self.contracts.update(contracts);

        Self { contracts, ..self }
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Position) -> bool {
        self.contract_symbol == other.contract_symbol
    }
}

impl Hash for Position {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.contract_symbol.hash(state);
    }
}

impl Borrow<ContractSymbol> for Position {
    fn borrow(&self) -> &ContractSymbol {
        &self.contract_symbol
    }
}
