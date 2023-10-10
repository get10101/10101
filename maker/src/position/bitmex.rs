use crate::position::ContractSymbol;
use std::borrow::Borrow;
use std::hash::Hash;
use std::hash::Hasher;

/// The maker's position on BitMEX.
#[derive(Clone, Eq, Debug)]
pub struct Position {
    contract_symbol: ContractSymbol,
    contracts: HundredsOfContracts,
}

/// Hundreds of contracts, with the sign representing the direction: positive long; negative short.
#[derive(Clone, PartialEq, Eq, Debug)]
struct HundredsOfContracts(i32);

impl HundredsOfContracts {
    pub fn new(contracts: i32) -> Self {
        let hundreds = contracts / 100;

        Self(hundreds)
    }
}

impl Position {
    pub fn new(contract_symbol: ContractSymbol) -> Self {
        Self {
            contract_symbol,
            contracts: HundredsOfContracts(0),
        }
    }

    pub fn update(&mut self, new_contracts: i32) {
        let before = self.contracts();

        self.contracts = HundredsOfContracts::new(new_contracts);

        let after = self.contracts();

        if before != after {
            tracing::info!(
                contract_symbol = ?self.contract_symbol,
                %before,
                %after,
                "Updated BitMEX position"
            );
        }
    }

    pub(super) fn contracts(&self) -> i32 {
        self.contracts.0 * 100
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
