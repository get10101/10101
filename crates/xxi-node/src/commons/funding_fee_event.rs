use crate::commons::ContractSymbol;
use crate::commons::Direction;
use bitcoin::SignedAmount;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FundingFeeEvent {
    pub contract_symbol: ContractSymbol,
    pub contracts: Decimal,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    /// A positive amount indicates that the trader pays the coordinator; a negative amount
    /// indicates that the coordinator pays the trader.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub fee: SignedAmount,
    pub due_date: OffsetDateTime,
}
