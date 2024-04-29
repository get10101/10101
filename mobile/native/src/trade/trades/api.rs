use bitcoin::SignedAmount;
use flutter_rust_bridge::frb;
use rust_decimal::prelude::ToPrimitive;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::Direction;

// TODO: Include fee rate.
#[frb]
#[derive(Debug, Clone)]
pub struct Trade {
    pub trade_type: TradeType,
    pub contract_symbol: ContractSymbol,
    pub contracts: f32,
    pub price: f32,
    /// Either a funding fee or an order-matching fee.
    pub fee: i64,
    /// Direction of the associated order.
    pub direction: Direction,
    /// Some trades may have a PNL associated with them.
    pub pnl: Option<i64>,
    pub timestamp: i64,
    pub is_done: bool,
}

#[frb]
#[derive(Debug, Clone)]
pub enum TradeType {
    Funding,
    Trade,
}

impl From<crate::trade::Trade> for Trade {
    fn from(value: crate::trade::Trade) -> Self {
        Self {
            trade_type: TradeType::Trade,
            contract_symbol: value.contract_symbol,
            contracts: value.contracts.to_f32().expect("to fit"),
            price: value.price.to_f32().expect("to fit"),
            fee: value.fee.to_sat() as i64,
            direction: value.direction,
            pnl: value.pnl.map(SignedAmount::to_sat),
            timestamp: value.timestamp.unix_timestamp(),
            is_done: true,
        }
    }
}

impl From<crate::trade::FundingFeeEvent> for Trade {
    fn from(value: crate::trade::FundingFeeEvent) -> Self {
        Self {
            trade_type: TradeType::Funding,
            contract_symbol: value.contract_symbol,
            contracts: value.contracts.to_f32().expect("to fit"),
            price: value.price.to_f32().expect("to fit"),
            fee: value.fee.to_sat(),
            direction: value.direction,
            pnl: None,
            timestamp: value.due_date.unix_timestamp(),
            is_done: value.paid_date.is_some(),
        }
    }
}
