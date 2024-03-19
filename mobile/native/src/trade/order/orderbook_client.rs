use crate::commons::reqwest_client;
use crate::ln_dlc::get_node_key;
use crate::trade::order::Order;
use crate::trade::order::OrderType;
use anyhow::bail;
use anyhow::Result;
use commons::ChannelOpeningParams;
use commons::MarginOrder;
use commons::MarketOrder;
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
        order: Order,
        channel_opening_params: Option<ChannelOpeningParams>,
    ) -> Result<()> {
        let order = match order.order_type {
            OrderType::Margin => NewOrder::Margin(MarginOrder::from(order)),
            OrderType::Market => NewOrder::Market(MarketOrder::from(order)),
            OrderType::Limit { .. } => {
                bail!("Limit orders are not yet implemented");
            }
        };

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
