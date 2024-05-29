use crate::commons::to_nearest_hour_in_the_past;
use crate::commons::ContractSymbol;
use crate::commons::Direction;
use bitcoin::SignedAmount;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

/// The funding rate for any position opened before the `end_date`, which remained open through the
/// `end_date`.
#[derive(Serialize, Clone, Copy, Deserialize, Debug)]
pub struct FundingRate {
    /// A positive funding rate indicates that longs pay shorts; a negative funding rate indicates
    /// that shorts pay longs.
    rate: Decimal,
    /// The start date for the funding rate period. This value is only used for informational
    /// purposes.
    ///
    /// The `start_date` is always a whole hour.
    start_date: OffsetDateTime,
    /// The end date for the funding rate period. When the end date has passed, all active
    /// positions that were created before the end date should be charged a funding fee based
    /// on the `rate`.
    ///
    /// The `end_date` is always a whole hour.
    end_date: OffsetDateTime,
}

impl FundingRate {
    pub fn new(rate: Decimal, start_date: OffsetDateTime, end_date: OffsetDateTime) -> Self {
        let start_date = to_nearest_hour_in_the_past(start_date);
        let end_date = to_nearest_hour_in_the_past(end_date);

        Self {
            rate,
            start_date,
            end_date,
        }
    }

    pub fn rate(&self) -> Decimal {
        self.rate
    }

    pub fn start_date(&self) -> OffsetDateTime {
        self.start_date
    }

    pub fn end_date(&self) -> OffsetDateTime {
        self.end_date
    }
}

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
