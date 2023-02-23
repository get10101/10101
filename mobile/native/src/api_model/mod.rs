pub mod order;
pub mod position;
use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use flutter_rust_bridge::frb;

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum ContractSymbol {
    BtcUsd,
}

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Long,
    Short,
}

impl From<Direction> for DirectionTrade {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Long => DirectionTrade::Long,
            Direction::Short => DirectionTrade::Short,
        }
    }
}

impl From<DirectionTrade> for Direction {
    fn from(value: DirectionTrade) -> Self {
        match value {
            DirectionTrade::Long => Direction::Long,
            DirectionTrade::Short => Direction::Short,
        }
    }
}

impl From<ContractSymbol> for ContractSymbolTrade {
    fn from(value: ContractSymbol) -> Self {
        match value {
            ContractSymbol::BtcUsd => ContractSymbolTrade::BtcUsd,
        }
    }
}

impl From<ContractSymbolTrade> for ContractSymbol {
    fn from(value: ContractSymbolTrade) -> Self {
        match value {
            ContractSymbolTrade::BtcUsd => ContractSymbol::BtcUsd,
        }
    }
}
