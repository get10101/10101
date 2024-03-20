use anyhow::Result;
use commons::ChannelOpeningParams;
use commons::NewOrder;
use commons::NewOrderRequest;
use reqwest::Client;
use reqwest::Url;
use secp256k1::SecretKey;

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
    ) -> Result<()> {
        tracing::info!(
            id = order.id.to_string(),
            direction = order.direction.to_string(),
            price = order.price.to_string(),
            "Posting order"
        );
        let message = order.message();
        let signature = secret_key.sign_ecdsa(message);
        let new_order_request = NewOrderRequest {
            value: order,
            signature,
            channel_opening_params,
        };

        let response = self
            .client
            .post(self.url.clone())
            .json(&new_order_request)
            .send()
            .await?;

        response.error_for_status()?;

        Ok(())
    }
}
