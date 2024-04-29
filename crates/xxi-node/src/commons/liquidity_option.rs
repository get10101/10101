use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityOption {
    pub id: i32,
    pub rank: usize,
    pub title: String,
    /// amount the trader can trade up to in sats
    pub trade_up_to_sats: u64,
    /// min deposit in sats
    pub min_deposit_sats: u64,
    /// max deposit in sats
    pub max_deposit_sats: u64,
    /// min fee in sats
    pub min_fee_sats: u64,
    pub fee_percentage: f64,
    pub coordinator_leverage: f32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub active: bool,
}

impl LiquidityOption {
    /// Get fees for the liquidity option on an amount in sats
    pub fn get_fee(&self, amount_sats: Decimal) -> Decimal {
        let fee = (amount_sats / Decimal::from(100))
            * Decimal::try_from(self.fee_percentage).expect("to fit into decimal");
        if fee < Decimal::from(self.min_fee_sats) {
            return Decimal::from(self.min_fee_sats);
        }
        fee
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingParam {
    pub target_node: String,
    pub user_channel_id: String,
    pub amount_sats: u64,
    pub liquidity_option_id: i32,
}

#[cfg(test)]
mod test {
    use crate::commons::liquidity_option::LiquidityOption;
    use rust_decimal::Decimal;
    use time::OffsetDateTime;

    fn get_liquidity_option() -> LiquidityOption {
        LiquidityOption {
            id: 1,
            rank: 1,
            title: "test".to_string(),
            trade_up_to_sats: 500_000,
            min_deposit_sats: 50_000,
            max_deposit_sats: 500_000,
            min_fee_sats: 10_000,
            fee_percentage: 1.0,
            coordinator_leverage: 2.0,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            active: true,
        }
    }

    #[test]
    fn test_min_fee() {
        let option = get_liquidity_option();
        let fee = option.get_fee(Decimal::from(60_000));
        assert_eq!(Decimal::from(10_000), fee)
    }

    #[test]
    fn test_percentage_fee() {
        let option = get_liquidity_option();
        let fee = option.get_fee(Decimal::from(1_100_000));
        assert_eq!(Decimal::from(11_000), fee)
    }
}
