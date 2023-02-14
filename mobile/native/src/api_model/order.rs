use crate::api_model::ContractSymbol;
use crate::api_model::Position;
use flutter_rust_bridge::frb;

#[frb]
#[derive(Debug, Clone, Copy)]
pub struct MarketOrder {
    #[frb(non_final)]
    pub leverage: f64,
    #[frb(non_final)]
    pub quantity: f64,
    #[frb(non_final)]
    pub margin: u64,
    #[frb(non_final)]
    pub contract_symbol: ContractSymbol,
    #[frb(non_final)]
    pub position: Position,
}
