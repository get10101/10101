use flutter_rust_bridge::frb;
pub mod order;

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum ContractSymbol {
    BtcUsd,
}

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum Position {
    Long,
    Short,
}
