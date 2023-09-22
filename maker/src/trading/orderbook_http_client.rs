use anyhow::bail;
use anyhow::Result;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderResponse;
use reqwest::Url;

pub struct OrderbookClient {
    client: reqwest::Client,
}

impl OrderbookClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("to build client from static config"),
        }
    }

    pub async fn post_new_order(&self, url: &Url, order: NewOrder) -> Result<OrderResponse> {
        let url = url.join("/api/orderbook/orders")?;

        let response = self.client.post(url).json(&order).send().await?;

        if response.status().as_u16() == 200 {
            let response = response.json().await?;
            Ok(response)
        } else {
            tracing::error!("Could not create new order");
            bail!("Could not create new order ")
        }
    }
}
