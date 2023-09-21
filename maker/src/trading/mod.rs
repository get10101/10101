use crate::health::ServiceStatus;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use futures::TryStreamExt;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderResponse;
use orderbook_commons::OrderType;
use orderbook_http_client::OrderbookClient;
use reqwest::Url;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use time::Duration;
use time::OffsetDateTime;
use tokio::sync::watch;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

mod bitmex_client;
mod orderbook_http_client;

/// Creates orders based of the current price feed from Bitmex.
///
/// In the unlikely event that the price feed is closed, the function will
/// continue to try to reconnect after Duration specified in `reconnect_after`.
pub async fn run(
    orderbook_url: &Url,
    maker_id: PublicKey,
    network: Network,
    concurrent_orders: usize,
    order_expiry_after: Duration,
    bitmex_pricefeed_tx: watch::Sender<ServiceStatus>,
    reconnect_after: std::time::Duration,
) {
    let network = match network {
        Network::Bitcoin => bitmex_stream::Network::Mainnet,
        _ => bitmex_stream::Network::Testnet,
    };

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
            OffsetDateTime::now_utc() + order_expiry_after,
        )
    };

    loop {
        let mut price_stream = bitmex_client::bitmex(network).await;
        while let Ok(Some(quote)) = price_stream.try_next().await {
            let _ = bitmex_pricefeed_tx.send(ServiceStatus::Online);
            tracing::debug!("Received new quote {quote:?}");

            // Clear stale orders. They should have expired by now.
            for order in orders.iter() {
                delete_order(&orderbook_client, orderbook_url, order).await;
            }
            orders.clear();

            for _i in 0..concurrent_orders {
                if let Some(order) = add_new_order(quote.bid(), Direction::Long).await {
                    orders.push(order)
                };

                if let Some(order) = add_new_order(quote.ask(), Direction::Short).await {
                    orders.push(order)
                };
            }
        }
        tracing::error!("Bitmex pricefeed stream closed");
        let _ = bitmex_pricefeed_tx.send(ServiceStatus::Offline);
        tokio::time::sleep(reconnect_after).await;
    }
}

async fn add_order(
    orderbook_client: &OrderbookClient,
    orderbook_url: &Url,
    price: Decimal,
    direction: Direction,
    maker_id: PublicKey,
    quantity: Decimal,
    expiry: OffsetDateTime,
) -> Option<OrderResponse> {
    orderbook_client
        .post_new_order(
            orderbook_url,
            NewOrder {
                id: Uuid::new_v4(),
                contract_symbol: ContractSymbol::BtcUsd,
                price,
                quantity,
                trader_id: maker_id,
                direction,
                leverage: 1.0,
                order_type: OrderType::Limit,
                expiry,
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
