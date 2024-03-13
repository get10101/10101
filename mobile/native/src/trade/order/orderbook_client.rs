use crate::commons::reqwest_client;
use crate::ln_dlc::get_node_key;
use anyhow::Result;
use commons::ChannelOpeningParams;
use commons::NewOrder;
use commons::NewOrderRequest;
use reqwest::Url;

pub struct OrderbookClient {
    url: Url,
}

impl OrderbookClient {
    pub fn new(url: Url) -> Self {
        Self { url }
    }

    pub(crate) async fn post_new_order(
        &self,
        order: NewOrder,
        channel_opening_params: Option<ChannelOpeningParams>,
    ) -> Result<()> {
        let secret_key = get_node_key();
        let message = order.message();
        let signature = secret_key.sign_ecdsa(message);
        let new_order_request = NewOrderRequest {
            value: order,
            signature,
            channel_opening_params,
        };

        let url = self.url.join("/api/orderbook/orders")?;
        let client = reqwest_client();

        let response = client.post(url).json(&new_order_request).send().await?;

        response.error_for_status()?;

        Ok(())
    }
}
