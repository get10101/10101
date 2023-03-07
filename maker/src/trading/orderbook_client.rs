use anyhow::bail;
use anyhow::Result;
use reqwest::Url;
use serde::Serialize;

#[derive(Serialize)]
pub struct Order {
    pub price: i32,
    pub maker_id: String,
    pub taken: bool,
}

pub async fn post_new_order(url: Url, maker_id: String) -> Result<()> {
    let url = url.join("/api/orderbook/orders")?;
    let client = reqwest::Client::new();
    let order = Order {
        price: 10,
        maker_id,
        taken: false,
    };

    let response = client.post(url).json(&order).send().await?;

    if response.status().as_u16() == 200 {
        Ok(())
    } else {
        tracing::error!("Could not create new order");
        bail!("Could not create new order ")
    }
}
