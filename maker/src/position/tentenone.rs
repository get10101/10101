use crate::position::ContractSymbol;
use crate::position::Contracts;
use rust_decimal::Decimal;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::hash::Hasher;
use uuid::Uuid;

/// Simulation of what the maker's position for a particular [`ContractSymbol`] would be if there
/// was a DLC channel between maker and coordinator.
///
/// TODO: Eventually, this should be a shadow representation of the position in the
/// maker-coordinator DLC channel.
#[derive(Clone, Eq, Debug)]
pub struct Position {
    contract_symbol: ContractSymbol,
    orders: HashMap<Uuid, Decimal>,
}

impl Position {
    pub fn new(contract_symbol: ContractSymbol) -> Self {
        Self {
            contract_symbol,
            orders: HashMap::default(),
        }
    }

    pub fn update(&mut self, order_id: Uuid, contracts: Decimal) {
        let before = self.contracts();

        match self.orders.insert(order_id, contracts) {
            Some(old_contracts) if old_contracts != contracts => {
                tracing::warn!(
                    %order_id,
                    %old_contracts,
                    new_contracts = %contracts,
                    "Updated 10101 contracts for existing order"
                );
            }
            Some(_) => {
                // Inconsequential update.
                return;
            }
            None => {
                // New order.
            }
        }

        let after = self.contracts();

        tracing::info!(
            contract_symbol = ?self.contract_symbol,
            %before,
            %after,
            %order_id,
            "Updated 10101 position"
        );
    }

    pub(super) fn contracts(&self) -> Contracts {
        self.orders
            .iter()
            .fold(Contracts::Balanced, |acc, (_, contracts)| {
                acc + Contracts::new(*contracts)
            })
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
