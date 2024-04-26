use crate::commons::Direction;
use crate::commons::Price;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Network;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::ops::Sub;
use std::time::Duration;
use time::format_description;
use time::OffsetDateTime;

pub struct BitmexClient {}

impl BitmexClient {
    /// gets a quote for a given timestamp. An error is returned if the provided timestamp is
    /// greater than the current timestamp
    pub async fn get_quote(network: &Network, timestamp: &OffsetDateTime) -> Result<Quote> {
        if OffsetDateTime::now_utc().lt(timestamp) {
            bail!("timestamp must not be in the future!")
        }

        let url = match network {
            Network::Bitcoin => "www.bitmex.com".to_string(),
            _ => "testnet.bitmex.com".to_string(),
        };

        let format = format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]")?;

        // subtracting a second from the start time to ensure we will get a quote from bitmex.
        let start_time = timestamp.sub(Duration::from_secs(60)).format(&format)?;
        let end_time = timestamp.format(&format)?;

        let quote: Vec<Quote> = reqwest::get(format!("https://{url}/api/v1/quote?symbol=XBTUSD&count=1&reverse=false&startTime={start_time}&endTime={end_time}"))
            .await?
            .json()
            .await?;

        let quote = quote.first().context("Did not get any quote from bitmex")?;
        Ok(quote.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
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

impl Quote {
    pub fn get_price_for_direction(self, direction: Direction) -> Decimal {
        let price = Price::from(self);
        price.get_price_for_direction(direction)
    }
}

impl From<Quote> for Price {
    fn from(value: Quote) -> Self {
        Price {
            bid: value.bid_price,
            ask: value.ask_price,
        }
    }
}
