use bdk::bitcoin::secp256k1::ecdsa::Signature;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::Network;
use bdk::bitcoin::Transaction;
use bdk::bitcoin::Txid;
use orderbook_commons::FilledWith;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use time::macros::time;
use time::Duration;
use time::OffsetDateTime;
use time::Weekday;
use trade::ContractSymbol;
use trade::Direction;

/// The trade parameters defining the trade execution
///
/// Emitted by the orderbook when a match is found.
/// Both trading parties will receive trade params and then request trade execution with said trade
/// parameters from the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeParams {
    /// The identity of the trader
    pub pubkey: PublicKey,

    /// The contract symbol for the trade to be set up
    pub contract_symbol: ContractSymbol,

    /// The leverage of the trader
    ///
    /// This has to correspond to our order's leverage.
    pub leverage: f32,

    /// The quantity of the trader
    ///
    /// For the trade set up with the coordinator it is the quantity of the contract.
    /// This quantity may be the complete quantity of an order or a fraction.
    pub quantity: f32,

    /// The direction of the trader
    ///
    /// The direction from the point of view of the trader.
    /// The coordinator takes the counter-position when setting up the trade.
    pub direction: Direction,

    /// The filling information from the orderbook
    ///
    /// This is used by the coordinator to be able to make sure both trading parties are acting.
    /// The `quantity` has to match the cummed up quantities of the matches in `filled_with`.
    pub filled_with: FilledWith,
}

impl TradeParams {
    pub fn average_execution_price(&self) -> Decimal {
        self.filled_with.average_execution_price()
    }
}

/// Registration details for enrolling into the beta program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParams {
    pub pubkey: PublicKey,
    pub email: Option<String>,
    pub nostr: Option<String>,
}

impl RegisterParams {
    pub fn is_valid(&self) -> bool {
        self.email.is_some() || self.nostr.is_some()
    }
}

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

/// LSP channel details
#[derive(Serialize, Deserialize)]
pub struct LspConfig {
    /// The fee rate to be used for the DLC contracts in sats/vbyte
    pub contract_tx_fee_rate: u64,

    // The liquidity options for onboarding
    pub liquidity_options: Vec<LiquidityOption>,
}

/// Calculates the next expiry timestamp based on the given timestamp and the network.
pub fn calculate_next_expiry(timestamp: OffsetDateTime, network: Network) -> OffsetDateTime {
    match network {
        // Calculates the expiry timestamp at the next Sunday at 3 pm UTC from a given offset date
        // time. If the argument falls in between Friday, 3 pm UTC and Sunday, 3pm UTC, the
        // expiry will be calculated to next weeks Sunday at 3 pm
        Network::Bitcoin => {
            let days = if is_eligible_for_rollover(timestamp, network)
                || timestamp.weekday() == Weekday::Sunday
            {
                // if the provided timestamp is in the rollover weekend or on a sunday, we expire
                // the sunday the week after.
                7 - timestamp.weekday().number_from_monday() + 7
            } else {
                7 - timestamp.weekday().number_from_monday()
            };
            let time = timestamp
                .date()
                .with_hms(15, 0, 0)
                .expect("to fit into time");

            (time + Duration::days(days as i64)).assume_utc()
        }
        // Calculates the expiry timestamp on the same day at midnight unless its already in
        // rollover then the next day midnight.
        _ => {
            if is_eligible_for_rollover(timestamp, network) {
                let after_tomorrow = timestamp.date() + Duration::days(2);
                after_tomorrow.midnight().assume_utc()
            } else {
                let tomorrow = timestamp.date() + Duration::days(1);
                tomorrow.midnight().assume_utc()
            }
        }
    }
}

/// Checks whether the provided expiry date is eligible for a rollover
pub fn is_eligible_for_rollover(timestamp: OffsetDateTime, network: Network) -> bool {
    match network {
        // Returns true if the given date falls in between Friday 15 pm UTC and Sunday 15 pm UTC
        Network::Bitcoin => match timestamp.weekday() {
            Weekday::Friday => timestamp.time() >= time!(15:00),
            Weekday::Saturday => true,
            Weekday::Sunday => timestamp.time() < time!(15:00),
            _ => false,
        },
        // Returns true if the timestamp is less than 8 hours from now
        _ => {
            let midnight = (OffsetDateTime::now_utc().date() + Duration::days(1))
                .midnight()
                .assume_utc();
            (midnight - timestamp) < Duration::hours(8)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::calculate_next_expiry;
    use crate::is_eligible_for_rollover;
    use crate::LiquidityOption;
    use bdk::bitcoin::Network;
    use rust_decimal::Decimal;
    use time::Duration;
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

    #[test]
    fn test_is_not_eligible_for_rollover() {
        // Wed Aug 09 2023 09:30:23 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691573423).unwrap();
        assert!(!is_eligible_for_rollover(expiry, Network::Bitcoin));
    }

    #[test]
    fn test_is_just_eligible_for_rollover_friday() {
        // Fri Aug 11 2023 15:00:00 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691766000).unwrap();
        assert!(is_eligible_for_rollover(expiry, Network::Bitcoin));

        // Fri Aug 11 2023 15:00:01 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691766001).unwrap();
        assert!(is_eligible_for_rollover(expiry, Network::Bitcoin));
    }

    #[test]
    fn test_is_eligible_for_rollover_saturday() {
        // Sat Aug 12 2023 16:00:00 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691856000).unwrap();
        assert!(is_eligible_for_rollover(expiry, Network::Bitcoin));
    }

    #[test]
    fn test_is_just_eligible_for_rollover_sunday() {
        // Sun Aug 13 2023 14:59:59 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691938799).unwrap();
        assert!(is_eligible_for_rollover(expiry, Network::Bitcoin));
    }

    #[test]
    fn test_is_just_not_eligible_for_rollover_sunday() {
        // Sun Aug 13 2023 15:00:00 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691938800).unwrap();
        assert!(!is_eligible_for_rollover(expiry, Network::Bitcoin));

        // Sun Aug 13 2023 15:00:01 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691938801).unwrap();
        assert!(!is_eligible_for_rollover(expiry, Network::Bitcoin));
    }

    #[test]
    fn test_expiry_timestamp_before_friday_15pm() {
        // Wed Aug 09 2023 09:30:23 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691573423).unwrap();
        let expiry = calculate_next_expiry(from, Network::Bitcoin);

        // Sun Aug 13 2023 15:00:00 GMT+0000
        assert_eq!(1691938800, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_just_before_friday_15pm() {
        // Fri Aug 11 2023 14:59:59 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691765999).unwrap();
        let expiry = calculate_next_expiry(from, Network::Bitcoin);

        // Sun Aug 13 2023 15:00:00 GMT+0000
        assert_eq!(1691938800, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_just_after_friday_15pm() {
        // Fri Aug 11 2023 15:00:01 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691766001).unwrap();
        let expiry = calculate_next_expiry(from, Network::Bitcoin);

        // Sun Aug 20 2023 15:00:00 GMT+0000
        assert_eq!(1692543600, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_at_friday_15pm() {
        // Fri Aug 11 2023 15:00:00 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691766000).unwrap();
        let expiry = calculate_next_expiry(from, Network::Bitcoin);

        // Sun Aug 20 2023 15:00:00 GMT+0000
        assert_eq!(1692543600, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_after_sunday_15pm() {
        // Sun Aug 06 2023 16:00:00 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691337600).unwrap();
        let expiry = calculate_next_expiry(from, Network::Bitcoin);

        // Sun Aug 13 2023 15:00:00 GMT+0000
        assert_eq!(1691938800, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_on_saturday() {
        // Sat Aug 12 2023 16:00:00 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691856000).unwrap();
        let expiry = calculate_next_expiry(from, Network::Bitcoin);

        // Sun Aug 20 2023 15:00:00 GMT+0000
        assert_eq!(1692543600, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_regtest_midnight() {
        // 12:00 on the current day
        let timestamp = OffsetDateTime::now_utc().date().midnight() + Duration::hours(12);
        let expiry = calculate_next_expiry(timestamp.assume_utc(), Network::Regtest);

        let midnight = (OffsetDateTime::now_utc().date() + Duration::days(1))
            .midnight()
            .assume_utc();

        assert_eq!(midnight, expiry);
    }

    #[test]
    fn test_expiry_timestamp_regtest_next_midnight() {
        // 20:00 on the current day
        let timestamp = OffsetDateTime::now_utc().date().midnight() + Duration::hours(20);
        let expiry = calculate_next_expiry(timestamp.assume_utc(), Network::Regtest);

        let next_midnight = (timestamp.date() + Duration::days(2))
            .midnight()
            .assume_utc();

        assert_eq!(next_midnight, expiry);
    }

    #[test]
    fn test_is_not_eligable_for_rollover_regtest() {
        let timestamp = OffsetDateTime::now_utc().date().midnight() + Duration::hours(16);
        assert!(!is_eligible_for_rollover(
            timestamp.assume_utc(),
            Network::Regtest
        ))
    }

    #[test]
    fn test_is_eligable_for_rollover_regtest() {
        let timestamp = OffsetDateTime::now_utc().date().midnight() + Duration::hours(17);
        assert!(is_eligible_for_rollover(
            timestamp.assume_utc(),
            Network::Regtest
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingParam {
    pub target_node: String,
    pub user_channel_id: String,
    pub amount_sats: u64,
    pub liquidity_option_id: i32,
}

#[derive(Deserialize, Serialize)]
pub struct CollaborativeRevert {
    /// Channel to collaboratively close
    pub channel_id: String,
    /// Price to calculate PnL for trader and coordinator
    pub price: Decimal,
    /// Fee rate for the closing transaction
    pub fee_rate_sats_vb: u64,
    /// The UTXO of the funding transaction
    pub txid: Txid,
    /// The vout of the funding transaction
    pub vout: u32,
}

#[derive(Deserialize, Serialize)]
pub struct CollaborativeRevertData {
    pub channel_id: String,
    /// the collaborative closing transaction unsigned
    pub transaction: Transaction,
    /// the trader's signature on the provided transaction
    pub signature: Signature,
}
