use bitcoin::SignedAmount;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::Direction;

pub mod handler;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FundingFeeEvent {
    pub contract_symbol: ContractSymbol,
    pub contracts: Decimal,
    pub direction: Direction,
    pub price: Decimal,
    /// A positive amount indicates that the trader pays the coordinator; a negative amount
    /// indicates that the coordinator pays the trader.
    pub fee: SignedAmount,
    pub due_date: OffsetDateTime,
    pub paid_date: Option<OffsetDateTime>,
}

impl FundingFeeEvent {
    pub fn unpaid(
        contract_symbol: ContractSymbol,
        contracts: Decimal,
        direction: Direction,
        price: Decimal,
        fee: SignedAmount,
        due_date: OffsetDateTime,
    ) -> Self {
        Self {
            contract_symbol,
            contracts,
            direction,
            price,
            fee,
            due_date,
            paid_date: None,
        }
    }
}

impl From<xxi_node::FundingFeeEvent> for FundingFeeEvent {
    fn from(value: xxi_node::FundingFeeEvent) -> Self {
        Self {
            contract_symbol: value.contract_symbol,
            contracts: value.contracts,
            direction: value.direction,
            price: value.price,
            fee: value.fee,
            due_date: value.due_date,
            paid_date: None,
        }
    }
}
