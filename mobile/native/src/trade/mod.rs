pub mod order;

#[derive(Debug, Clone, Copy)]
pub enum ContractSymbolTrade {
    BtcUsd,
}

#[derive(Debug, Clone, Copy)]
pub enum DirectionTrade {
    Long,
    Short,
}
