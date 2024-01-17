use anyhow::anyhow;
use anyhow::Result;
use async_stream::stream;
use bitmex_stream::Credentials;
use bitmex_stream::Network;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use rust_decimal::Decimal;
use std::fmt;
use time::OffsetDateTime;
use trade::ContractSymbol;

pub async fn stream(
    network: Network,
    credentials: Option<Credentials>,
) -> impl Stream<Item = Result<Event>> + Unpin {
    let stream = stream! {
        let mut stream = match credentials {
            Some(credentials) => {
                bitmex_stream::subscribe_with_credentials(
                    ["quoteBin1m:XBTUSD".to_owned(), "position:XBTUSD".to_owned()],
                    network,
                    credentials
                ).boxed()
            }
            None => {
                bitmex_stream::subscribe(
                    ["quoteBin1m:XBTUSD".to_owned()],
                    network,
                ).boxed()
            }
        };

        loop {
            match stream.try_next().await {
                Ok(Some(text)) => {
                    match serde_json::from_str::<wire::TableUpdate>(&text) {
                        Ok(update) => {
                            let event = Event::from(update);

                            tracing::debug!(?event, "Received new event");

                            yield Ok(event);

                        }
                        Err(_) => {
                            tracing::debug!("Unexpected table update: {text}");
                        }
                    }
                },
                Err(error) => {
                    yield Err(error);
                }
                Ok(None) => {
                    yield Err(anyhow!("Stream ended"));
                }
            };
        }
    };

    stream.boxed()
}

#[derive(Debug, Clone)]
pub enum Event {
    Quote(Quote),
    Position(Position),
}

impl From<wire::TableUpdate> for Event {
    fn from(value: wire::TableUpdate) -> Self {
        match value {
            wire::TableUpdate::QuoteBin1m(quote) => Self::Quote(Quote {
                contract_symbol: quote.symbol.into(),
                bid: quote.bid_price,
                ask: quote.ask_price,
                timestamp: quote.timestamp,
            }),
            wire::TableUpdate::Position(position) => Self::Position(Position {
                contract_symbol: position.symbol.into(),
                contracts: position.contracts,
                timestamp: position.timestamp,
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Quote {
    pub contract_symbol: ContractSymbol,
    pub bid: Decimal,
    pub ask: Decimal,
    pub timestamp: OffsetDateTime,
}

impl fmt::Debug for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rfc3339_timestamp = self
            .timestamp
            .format(&time::format_description::well_known::Rfc3339)
            .expect("Timestamp to be formatted");

        f.debug_struct("Quote")
            .field("timestamp", &rfc3339_timestamp)
            .field("bid", &self.bid)
            .field("ask", &self.ask)
            .field("contract_symbol", &self.contract_symbol)
            .finish()
    }
}

impl Quote {
    pub fn bid(&self) -> Decimal {
        self.bid
    }

    pub fn ask(&self) -> Decimal {
        self.ask
    }

    #[allow(dead_code)]
    pub fn is_older_than(&self, duration: time::Duration) -> bool {
        let required_quote_timestamp = (OffsetDateTime::now_utc() - duration).unix_timestamp();

        self.timestamp.unix_timestamp() < required_quote_timestamp
    }
}

#[derive(Clone, Copy)]
pub struct Position {
    pub contract_symbol: ContractSymbol,
    pub contracts: i32,
    pub timestamp: OffsetDateTime,
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rfc3339_timestamp = self
            .timestamp
            .format(&time::format_description::well_known::Rfc3339)
            .expect("Timestamp to be formatted");

        f.debug_struct("Quote")
            .field("contract_symbol", &self.contract_symbol)
            .field("contracts", &self.contracts)
            .field("timestamp", &rfc3339_timestamp)
            .finish()
    }
}

mod wire {
    use core::fmt;
    use rust_decimal::Decimal;
    use serde::Deserialize;
    use serde::Deserializer;
    use time::OffsetDateTime;

    #[derive(Debug)]
    pub enum TableUpdate {
        QuoteBin1m(QuoteData),
        Position(PositionData),
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct QuoteData {
        pub bid_size: u64,
        pub ask_size: u64,
        #[serde(with = "rust_decimal::serde::float")]
        pub bid_price: Decimal,
        #[serde(with = "rust_decimal::serde::float")]
        pub ask_price: Decimal,
        pub symbol: ContractSymbol,
        #[serde(with = "time::serde::rfc3339")]
        pub timestamp: OffsetDateTime,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    pub struct PositionData {
        pub symbol: ContractSymbol,
        #[serde(rename = "currentQty")]
        pub contracts: i32,
        #[serde(with = "time::serde::rfc3339")]
        pub timestamp: OffsetDateTime,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    pub enum ContractSymbol {
        #[serde(rename = "XBTUSD")]
        XbtUsd,
    }

    impl From<ContractSymbol> for trade::ContractSymbol {
        fn from(value: ContractSymbol) -> Self {
            match value {
                ContractSymbol::XbtUsd => Self::BtcUsd,
            }
        }
    }

    impl<'de> Deserialize<'de> for TableUpdate {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct Visitor;

            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = TableUpdate;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("either a `QuoteBin1m` or a `Position` table update")
                }

                fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    #[derive(Debug)]
                    enum TableUpdateKind {
                        QuoteBin1m,
                        Position,
                    }

                    let mut table = None;
                    let mut data = None;

                    while let Some(key) = map.next_key()? {
                        match key {
                            "table" => {
                                if table.is_some() {
                                    return Err(serde::de::Error::duplicate_field("table"));
                                }

                                let value = match map.next_value()? {
                                    "quoteBin1m" => TableUpdateKind::QuoteBin1m,
                                    "position" => TableUpdateKind::Position,
                                    _ => return Err(serde::de::Error::custom("unexpected table")),
                                };

                                table = Some(value);
                            }
                            "data" => {
                                if data.is_some() {
                                    return Err(serde::de::Error::duplicate_field("data"));
                                }

                                // `serde_json::RawValue` here so that we can defer the decision of
                                // which concrete type to deserialise into when we've gone through
                                // every key of the map and
                                data = Some(map.next_value::<&serde_json::value::RawValue>()?);
                            }
                            _ => {
                                map.next_value::<serde::de::IgnoredAny>()?;
                            }
                        }
                    }

                    let table = table.ok_or_else(|| serde::de::Error::missing_field("table"))?;
                    let data = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;

                    // Now that we know the type of table we're dealing with we can choose between
                    // the variants we support
                    let value = match table {
                        TableUpdateKind::QuoteBin1m => TableUpdate::QuoteBin1m(
                            serde_json::from_str::<[QuoteData; 1]>(data.get()).map_err(|e| {
                                serde::de::Error::custom(format!(
                                    "could not deserialize quote data: {e}"
                                ))
                            })?[0]
                                .clone(),
                        ),
                        TableUpdateKind::Position => TableUpdate::Position(
                            serde_json::from_str::<[PositionData; 1]>(data.get()).map_err(|e| {
                                serde::de::Error::custom(format!(
                                    "could not deserialize position data: {e}"
                                ))
                            })?[0]
                                .clone(),
                        ),
                    };

                    Ok(value)
                }
            }

            deserializer.deserialize_map(Visitor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::ext::NumericalDuration;

    #[test]
    fn can_deserialize_quote_update() {
        let table_update = serde_json::from_str(r#"{"table":"quoteBin1m","action":"insert","data":[{"timestamp":"2021-09-21T02:40:00.000Z","symbol":"XBTUSD","bidSize":50200,"bidPrice":42640.5,"askPrice":42641,"askSize":363600}]}"#).unwrap();

        match table_update {
            wire::TableUpdate::QuoteBin1m(wire::QuoteData {
                bid_size,
                ask_size,
                bid_price,
                ask_price,
                symbol,
                timestamp,
            }) => {
                assert_eq!(symbol, wire::ContractSymbol::XbtUsd);
                assert_eq!(bid_size, 50200);
                assert_eq!(ask_size, 363600);
                assert_eq!(bid_price, dec!(42640.5));
                assert_eq!(ask_price, dec!(42641));
                assert_eq!(timestamp.unix_timestamp(), 1632192000);
            }
            _ => panic!("Unexpected table update"),
        }
    }

    #[test]
    fn can_deserialize_position_update() {
        let table_update = serde_json::from_str(r#"{"table":"position","action":"update","data":[{"account":396867,"symbol":"XBTUSD","currency":"XBt","currentQty":100,"markPrice":27452.26,"markValue":-364269,"riskValue":364269,"homeNotional":0.00364269,"maintMargin":65585,"unrealisedPnl":-6327,"unrealisedPnlPcnt":-0.0177,"unrealisedRoePcnt":-0.0884,"liquidationPrice":23349.5,"timestamp":"2023-10-05T17:36:45.781Z"}]}"#).unwrap();

        match table_update {
            wire::TableUpdate::Position(wire::PositionData {
                symbol,
                contracts,
                timestamp,
            }) => {
                assert_eq!(symbol, wire::ContractSymbol::XbtUsd);
                assert_eq!(contracts, 100);
                assert_eq!(timestamp.unix_timestamp(), 1696527405)
            }
            _ => panic!("Unexpected table update"),
        }
    }

    #[test]
    fn quote_from_now_is_not_old() {
        let quote = dummy_quote_at(OffsetDateTime::now_utc());

        let is_older = quote.is_older_than(1.minutes());

        assert!(!is_older)
    }

    #[test]
    fn quote_from_one_hour_ago_is_old() {
        let quote = dummy_quote_at(OffsetDateTime::now_utc() - 1.hours());

        let is_older = quote.is_older_than(1.minutes());

        assert!(is_older)
    }

    fn dummy_quote_at(timestamp: OffsetDateTime) -> Quote {
        Quote {
            timestamp,
            bid: dec!(10),
            ask: dec!(10),
            contract_symbol: trade::ContractSymbol::BtcUsd,
        }
    }
}
