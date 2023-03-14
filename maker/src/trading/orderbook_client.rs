use anyhow::bail;
use anyhow::Result;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderResponse;
use reqwest::Url;

pub async fn post_new_order(url: Url, order: NewOrder) -> Result<OrderResponse> {
    let url = url.join("/api/orderbook/orders")?;
    let client = reqwest::Client::new();

    let response = client.post(url).json(&order).send().await?;

    if response.status().as_u16() == 200 {
        let response = response.json().await?;
        Ok(response)
    } else {
        tracing::error!("Could not create new order");
        bail!("Could not create new order ")
    }
}

pub async fn delete_order(url: Url, order_id: i32) -> Result<()> {
    let url = url.join(format!("/api/orderbook/orders/{order_id}").as_str())?;
    let client = reqwest::Client::new();

    let response = client.delete(url).send().await?;

    if response.status().as_u16() == 200 {
        Ok(())
    } else {
        tracing::error!("Could not delete new order");
        bail!("Could not create new order ")
    }
}
