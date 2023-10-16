use crate::health::ServiceStatus;
use crate::position;
use crate::position::PositionUpdateBitmex;
use crate::trading::bitmex_ws_client::Event;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use bitmex_stream::Credentials;
use futures::TryStreamExt;
use orderbook_commons::NewOrder;
use orderbook_commons::OrderResponse;
use orderbook_commons::OrderType;
use orderbook_http_client::OrderbookClient;
use reqwest::Url;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::watch;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

mod bitmex_ws_client;
mod orderbook_http_client;

/// Perform trading related actions based on a subscription to BitMEX's WebSocket API. Specifically:
///
/// - Create orders based on relevant price updates from BitMEX.
/// - Forward updates about all BitMEX positions.
///
/// In the unlikely event that the stream is closed, the function will continue to try to reconnect
/// after the [`Duration`] specified by `reconnect_after`.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    orderbook_url: &Url,
    maker_id: PublicKey,
    network: Network,
    concurrent_orders: usize,
    order_expiry_after: time::Duration,
    bitmex_pricefeed_tx: watch::Sender<ServiceStatus>,
    position_manager: xtra::Address<position::Manager>,
    bitmex_api_key: Option<String>,
    bitmex_api_secret: Option<String>,
    reconnect_after: Duration,
) {
    let network = match network {
        Network::Bitcoin => bitmex_stream::Network::Mainnet,
        _ => bitmex_stream::Network::Testnet,
    };

    let orderbook_client = OrderbookClient::new();

    let mut orders: Vec<OrderResponse> = Vec::new();

    // Closure to avoid repeating the same code
    let add_new_10101_order = |price, direction| {
        add_10101_order(
            &orderbook_client,
            orderbook_url,
            price,
            direction,
            maker_id,
            dec!(1000),
            OffsetDateTime::now_utc() + order_expiry_after,
        )
    };

    let credentials = match (bitmex_api_key, bitmex_api_secret) {
        (Some(api_key), Some(secret)) => Some(Credentials { api_key, secret }),
        _ => None,
    };

    loop {
        let mut stream = bitmex_ws_client::stream(network, credentials.clone()).await;
        loop {
            match stream.try_next().await {
                Ok(Some(Event::Quote(quote))) => {
                    let _ = bitmex_pricefeed_tx.send(ServiceStatus::Online);
                    tracing::debug!("Received new quote {quote:?}");

                    orders.clear();

                    for _i in 0..concurrent_orders {
                        if let Some(order) = add_new_10101_order(quote.bid(), Direction::Long).await
                        {
                            orders.push(order)
                        };

                        if let Some(order) =
                            add_new_10101_order(quote.ask(), Direction::Short).await
                        {
                            orders.push(order)
                        };
                    }
                }
                Ok(Some(Event::Position(position))) => {
                    let _ = position_manager
                        .send(PositionUpdateBitmex {
                            contract_symbol: position.contract_symbol.into(),
                            contracts: position.contracts,
                        })
                        .await;
                }
                Err(e) => {
                    tracing::error!("Closing BitMEX WS after encountering error: {e:#}");
                    break;
                }
                Ok(None) => {
                    tracing::error!("BitMEX WS closed");
                    break;
                }
            }
        }

        let _ = bitmex_pricefeed_tx.send(ServiceStatus::Offline);

        tracing::error!(timeout = ?reconnect_after, "Reconnecting to BitMEX WS after timeout");

        tokio::time::sleep(reconnect_after).await;
    }
}

async fn add_10101_order(
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
                stable: false,
            },
        )
        .await
        .map_err(|err| {
            tracing::error!("Failed posting new order {err:#}");
            err
        })
        .ok()
}
