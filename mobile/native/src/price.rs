use crate::api::Event;
use crate::config::maker_endpoint;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use flutter_rust_bridge::StreamSink;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use tokio::task::JoinHandle;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarketPrice {
    pub bid: f64,
    pub ask: f64,
    pub index: f64,
}

pub fn spawn(stream: StreamSink<Event>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let offer = get_offer().await.ok();
            stream.add(Event::Offer(offer));
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    })
}

pub async fn get_offer() -> Result<MarketPrice> {
    let dummy_price = MarketPrice {
        bid: 22.990,
        ask: 23.010,
        index: 23.000,
    };

    Ok(dummy_price)
}
