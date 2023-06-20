use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use futures::TryStreamExt;
use orderbook_client::OrderbookClient;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderResponse;
use orderbook_commons::OrderType;
use reqwest::Url;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use time::Duration;
use time::OffsetDateTime;
use trade::Direction;
use uuid::Uuid;

mod bitmex_client;
mod orderbook_client;

pub async fn run(orderbook_url: &Url, maker_id: PublicKey, network: Network) -> Result<()> {
    let network = match network {
        Network::Bitcoin => bitmex_stream::Network::Mainnet,
        _ => bitmex_stream::Network::Testnet,
    };
    let mut price_stream = bitmex_client::bitmex(network).await;

    let orderbook_client = OrderbookClient::new();

    let mut orders: Vec<OrderResponse> = Vec::new();

    // Closure to avoid repeating the same code
    let add_new_order = |price, direction| {
        add_order(
            &orderbook_client,
            orderbook_url,
            price,
            direction,
            maker_id,
            dec!(1000),
        )
    };

    while let Some(quote) = price_stream.try_next().await? {
        tracing::debug!("Received new quote {quote:?}");

        for order in orders.iter() {
            delete_order(&orderbook_client, orderbook_url, order).await;
        }
        orders.clear();

        if let Some(order) = add_new_order(quote.bid(), Direction::Long).await {
            orders.push(order)
        };

        if let Some(order) = add_new_order(quote.ask(), Direction::Short).await {
            orders.push(order)
        };
    }

    Ok(())
}

async fn add_order(
    orderbook_client: &OrderbookClient,
    orderbook_url: &Url,
    price: Decimal,
    direction: Direction,
    maker_id: PublicKey,
    quantity: Decimal,
) -> Option<OrderResponse> {
    orderbook_client
        .post_new_order(
            orderbook_url,
            NewOrder {
                id: Uuid::new_v4(),
                price,
                quantity,
                trader_id: maker_id,
                direction,
                order_type: OrderType::Limit,
                expiry: OffsetDateTime::now_utc() + Duration::minutes(1),
            },
        )
        .await
        .map_err(|err| {
            tracing::error!("Failed posting new order {err:#}");
            err
        })
        .ok()
}

async fn delete_order(
    orderbook_client: &OrderbookClient,
    orderbook_url: &Url,
    last_order: &OrderResponse,
) {
    let order_id = last_order.id;
    if let Err(err) = orderbook_client.delete_order(orderbook_url, order_id).await {
        tracing::error!("Failed deleting old order `{order_id}` because of {err:#}");
    }
}
