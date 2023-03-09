use anyhow::Error;
use anyhow::Result;
use async_stream::stream;
use bitmex_stream::Network;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;
use trade::ContractSymbol;

pub async fn bitmex(network: Network) -> impl Stream<Item = Result<Quote, Error>> + Unpin {
    let stream = stream! {
        loop {
            let mut stream = bitmex_stream::subscribe(["quoteBin1m:XBTUSD".to_owned()], network);

            loop {
                match stream.try_next().await {
                    Ok(Some(result)) => {
                        if let Some(quote) = handle_stream_msg(result) {
                            yield Ok(quote);
                        }
                    }
                    Err(error) => {
                        tracing::error!("Could not get result from BitMEX {error:#}");
                        break;
                    }
                    Ok(None) => {
                        // ignore
                    }
                }
            }

            let seconds = 10;
            tracing::warn!("Disconnected from BitMEX. Reconnecting to BitMEX in {seconds}");
            tokio::time::sleep(Duration::from_secs(seconds)).await;
        }
    };

    stream.boxed()
}

fn handle_stream_msg(text: String) -> Option<Quote> {
    match Quote::from_str(&text) {
        Ok(Some(quote)) => {
            tracing::debug!(bid = %quote.bid,
                ask = %quote.ask,
                timestamp = %quote.timestamp,
                symbol = %quote.symbol,
                "Received new quote",
            );
            return Some(quote);
        }
        Err(err) => {
            tracing::warn!("Could not deserialize quote {err}");
        }
        Ok(None) => {
            // ignore
        }
    }
    None
}

#[derive(Clone, Copy)]
pub struct Quote {
    pub timestamp: OffsetDateTime,
    pub bid: Decimal,
    pub ask: Decimal,
    pub symbol: ContractSymbol,
}

impl fmt::Debug for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rfc3339_timestamp = self
            .timestamp
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();

        f.debug_struct("Quote")
            .field("timestamp", &rfc3339_timestamp)
            .field("bid", &self.bid)
            .field("ask", &self.ask)
            .finish()
    }
}

impl Quote {
    fn from_str(text: &str) -> Result<Option<Self>> {
        let table_message = match serde_json::from_str::<wire::TableMessage>(text) {
            Ok(table_message) => table_message,
            Err(_) => {
                tracing::trace!(%text, "Not a 'table' message, skipping...");
                return Ok(None);
            }
        };

        let [quote] = table_message.data;

        let symbol = ContractSymbol::from_str(quote.symbol.as_str())?;
        Ok(Some(Self {
            timestamp: quote.timestamp,
            bid: quote.bid_price,
            ask: quote.ask_price,
            symbol,
        }))
    }

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

mod wire {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    pub struct TableMessage {
        pub table: String,
        // we always just expect a single quote, hence the use of an array instead of a vec
        pub data: [QuoteData; 1],
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
        pub symbol: String,
        #[serde(with = "time::serde::rfc3339")]
        pub timestamp: OffsetDateTime,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::ext::NumericalDuration;

    #[test]
    fn can_deserialize_quote_message() {
        let quote = Quote::from_str(r#"{"table":"quoteBin1m","action":"insert","data":[{"timestamp":"2021-09-21T02:40:00.000Z","symbol":"XBTUSD","bidSize":50200,"bidPrice":42640.5,"askPrice":42641,"askSize":363600}]}"#).unwrap().unwrap();

        assert_eq!(quote.bid, dec!(42640.5));
        assert_eq!(quote.ask, dec!(42641));
        assert_eq!(quote.timestamp.unix_timestamp(), 1632192000);
        assert_eq!(quote.symbol, ContractSymbol::BtcUsd)
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
            symbol: ContractSymbol::BtcUsd,
        }
    }
}
