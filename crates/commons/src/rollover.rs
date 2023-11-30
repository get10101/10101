use bitcoin::Network;
use time::macros::time;
use time::Duration;
use time::OffsetDateTime;
use time::Weekday;

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
    use crate::rollover::calculate_next_expiry;
    use crate::rollover::is_eligible_for_rollover;
    use bitcoin::Network;
    use time::Duration;
    use time::OffsetDateTime;

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
