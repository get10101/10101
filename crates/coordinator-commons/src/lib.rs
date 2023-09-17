use bdk::bitcoin::secp256k1::PublicKey;
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

/// LSP channel details
#[derive(Serialize, Deserialize)]
pub struct LspConfig {
    /// The maximum size a new channel may have
    pub max_channel_value_satoshi: u64,

    /// The fee rate to be used for the DLC contracts in sats/vbyte
    pub contract_tx_fee_rate: u64,
}
/// FCM token update parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUpdateParams {
    pub pubkey: String,
    pub fcm_token: String,
}

/// Calculates the expiry timestamp at the next Sunday at 3 pm UTC from a given offset date time.
/// If the argument falls in between Friday, 3 pm UTC and Sunday, 3pm UTC, the expiry will be
/// calculated to next weeks Sunday at 3 pm
pub fn calculate_next_expiry(time: OffsetDateTime) -> OffsetDateTime {
    let days = if is_in_rollover_weekend(time) || time.weekday() == Weekday::Sunday {
        // if the provided time is in the rollover weekend or on a sunday, we expire the sunday the
        // week after.
        7 - time.weekday().number_from_monday() + 7
    } else {
        7 - time.weekday().number_from_monday()
    };
    let time = time.date().with_hms(15, 0, 0).expect("to fit into time");

    (time + Duration::days(days as i64)).assume_utc()
}

/// Checks whether the provided expiry date is eligible for a rollover
///
/// Returns true if the given date falls in between friday 15 pm UTC and sunday 15 pm UTC
pub fn is_in_rollover_weekend(timestamp: OffsetDateTime) -> bool {
    match timestamp.weekday() {
        Weekday::Friday => timestamp.time() >= time!(15:00),
        Weekday::Saturday => true,
        Weekday::Sunday => timestamp.time() < time!(15:00),
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use crate::calculate_next_expiry;
    use crate::is_in_rollover_weekend;
    use time::OffsetDateTime;

    #[test]
    fn test_is_not_in_rollover_weekend() {
        // Wed Aug 09 2023 09:30:23 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691573423).unwrap();
        assert!(!is_in_rollover_weekend(expiry));
    }

    #[test]
    fn test_is_just_in_rollover_weekend_friday() {
        // Fri Aug 11 2023 15:00:00 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691766000).unwrap();
        assert!(is_in_rollover_weekend(expiry));

        // Fri Aug 11 2023 15:00:01 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691766001).unwrap();
        assert!(is_in_rollover_weekend(expiry));
    }

    #[test]
    fn test_is_in_rollover_weekend_saturday() {
        // Sat Aug 12 2023 16:00:00 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691856000).unwrap();
        assert!(is_in_rollover_weekend(expiry));
    }

    #[test]
    fn test_is_just_in_rollover_weekend_sunday() {
        // Sun Aug 13 2023 14:59:59 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691938799).unwrap();
        assert!(is_in_rollover_weekend(expiry));
    }

    #[test]
    fn test_is_just_not_in_rollover_weekend_sunday() {
        // Sun Aug 13 2023 15:00:00 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691938800).unwrap();
        assert!(!is_in_rollover_weekend(expiry));

        // Sun Aug 13 2023 15:00:01 GMT+0000
        let expiry = OffsetDateTime::from_unix_timestamp(1691938801).unwrap();
        assert!(!is_in_rollover_weekend(expiry));
    }

    #[test]
    fn test_expiry_timestamp_before_friday_15pm() {
        // Wed Aug 09 2023 09:30:23 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691573423).unwrap();
        let expiry = calculate_next_expiry(from);

        // Sun Aug 13 2023 15:00:00 GMT+0000
        assert_eq!(1691938800, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_just_before_friday_15pm() {
        // Fri Aug 11 2023 14:59:59 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691765999).unwrap();
        let expiry = calculate_next_expiry(from);

        // Sun Aug 13 2023 15:00:00 GMT+0000
        assert_eq!(1691938800, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_just_after_friday_15pm() {
        // Fri Aug 11 2023 15:00:01 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691766001).unwrap();
        let expiry = calculate_next_expiry(from);

        // Sun Aug 20 2023 15:00:00 GMT+0000
        assert_eq!(1692543600, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_at_friday_15pm() {
        // Fri Aug 11 2023 15:00:00 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691766000).unwrap();
        let expiry = calculate_next_expiry(from);

        // Sun Aug 20 2023 15:00:00 GMT+0000
        assert_eq!(1692543600, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_after_sunday_15pm() {
        // Sun Aug 06 2023 16:00:00 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691337600).unwrap();
        let expiry = calculate_next_expiry(from);

        // Sun Aug 13 2023 15:00:00 GMT+0000
        assert_eq!(1691938800, expiry.unix_timestamp());
    }

    #[test]
    fn test_expiry_timestamp_on_saturday() {
        // // Sat Aug 12 2023 16:00:00 GMT+0000
        let from = OffsetDateTime::from_unix_timestamp(1691856000).unwrap();
        let expiry = calculate_next_expiry(from);

        // Sun Aug 20 2023 15:00:00 GMT+0000
        assert_eq!(1692543600, expiry.unix_timestamp());
    }
}
