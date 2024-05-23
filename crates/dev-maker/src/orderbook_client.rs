use anyhow::Result;
use reqwest::Client;
use reqwest::Url;
use secp256k1::SecretKey;
use uuid::Uuid;
use xxi_node::commons::ChannelOpeningParams;
use xxi_node::commons::NewOrder;
use xxi_node::commons::NewOrderRequest;

#[derive(Clone)]
pub struct OrderbookClient {
    url: Url,
    client: Client,
}

impl OrderbookClient {
    pub fn new(url: Url) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build reqwest client");
        Self { url, client }
    }

    pub(crate) async fn post_new_order(
        &self,
        order: NewOrder,
        channel_opening_params: Option<ChannelOpeningParams>,
        secret_key: SecretKey,
        pre_image: Option<String>,
    ) -> Result<()> {
        let url = self.url.join("/api/orderbook/orders")?;

        tracing::info!(
            id = order.id().to_string(),
            direction = order.direction().to_string(),
            price = order.price().to_string(),
            "Posting order"
        );
        let message = order.message();
        let signature = secret_key.sign_ecdsa(message);
        let new_order_request = NewOrderRequest {
            value: order,
            signature,
            channel_opening_params,
            pre_image,
        };

        let response = self
            .client
            .post(url)
            .json(&new_order_request)
            .send()
            .await?;

        response.error_for_status()?;

        Ok(())
    }

    pub async fn delete_order(&self, order_id: &Uuid) -> Result<()> {
        tracing::debug!(
            order_id = order_id.to_string(),
            "Deleting order from orderbook"
        );

        let url = self.url.join(
            format!("/api/admin/orderbook/orders/{}", order_id)
                .to_string()
                .as_str(),
        )?;

        let response = self.client.delete(url).send().await?;

        response.error_for_status()?;

        Ok(())
    }
}
