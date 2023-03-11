use crate::orderbook;
use crate::routes::AppError;
use crate::routes::AppState;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::Path;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use bitcoin::secp256k1::PublicKey;
use futures::SinkExt;
use futures::StreamExt;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Sender;
use trade::Direction;
use trade::NewOrder;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub id: i32,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    pub maker_id: String,
    pub taken: bool,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
}

pub async fn get_orders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let order = orderbook::db::orders::all(&mut conn).unwrap();

    Json(order)
}

pub async fn get_order(
    Path(order_id): Path<i32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let pool = state.pool.clone();
    let mut conn = pool.get().unwrap();
    let order = orderbook::db::orders::get_with_id(&mut conn, order_id)
        .unwrap()
        .unwrap();

    Json(order)
}

pub async fn post_order(
    State(state): State<Arc<AppState>>,
    Json(new_order): Json<NewOrder>,
) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let order = orderbook::db::orders::insert(&mut conn, new_order).unwrap();

    let sender = state.tx_pricefeed.clone();
    update_pricefeed(PriceFeedMessage::NewOrder(order.clone()), sender);

    Json(order)
}

pub async fn fake_match(
    taker_pub_key: Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let taker_pub_key = taker_pub_key.0;
    let taker_pub_key: PublicKey = taker_pub_key.parse().map_err(|e| {
        AppError::BadRequest(format!(
            "Provided public key {taker_pub_key} was not valid: {e:#}"
        ))
    })?;

    // TODO: remove this dummy call to the coordinator, once the orderbook matching has been
    // implemented. Also this call should not happen here, it is just added for temporary
    // testing.

    tracing::debug!("Executing fake trade with hard coded trading parties.");

    let dummy_match_params = trade::MatchParams {
        taker: trade::Trade {
            pub_key: taker_pub_key,
            leverage: 1.0, // todo: the order book will know
            direction: Direction::Long,
            order_id: Uuid::parse_str("02f09a3f-1624-3b1d-8409-44eff7708208").unwrap(),
        },
        maker: trade::Trade {
            pub_key: "02b103838b4fc38a423342e2d187de6da76dde13e7a0271c8247e19c91027140f7"
                .parse()
                .unwrap(),
            leverage: 1.0, // todo: the order book will know
            direction: Direction::Short,
            order_id: Uuid::parse_str("02f09a3f-1624-3b1d-8409-33eff7708210").unwrap(), /* todo: the orderbook will know */
        },
        params: trade::Match {
            quantity: 1.0,
            execution_price: 22_000.0,
            expiry: Duration::from_secs(60 * 60 * 24), // in 24h
            contract_symbol: trade::ContractSymbol::BtcUsd,
        },
    };

    state.node.trade(dummy_match_params).await.unwrap();
    Ok(())
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum PriceFeedMessage {
    AllOrders(Vec<Order>),
    NewOrder(Order),
    DeleteOrder(i32),
    Update(Order),
}

fn update_pricefeed(pricefeed_msg: PriceFeedMessage, sender: Sender<PriceFeedMessage>) {
    match sender.send(pricefeed_msg) {
        Ok(_) => {
            tracing::trace!("Pricefeed updated")
        }
        Err(error) => {
            tracing::warn!("Could not update pricefeed due to '{error}'")
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct UpdateOrder {
    pub taken: bool,
}

pub async fn put_order(
    Path(order_id): Path<i32>,
    State(state): State<Arc<AppState>>,
    Json(updated_order): Json<UpdateOrder>,
) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let order = orderbook::db::orders::update(&mut conn, order_id, updated_order.taken).unwrap();
    let sender = state.tx_pricefeed.clone();
    update_pricefeed(PriceFeedMessage::Update(order.clone()), sender);

    Json(order)
}

pub async fn delete_order(
    Path(order_id): Path<i32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let deleted = orderbook::db::orders::delete_with_id(&mut conn, order_id).unwrap();
    if deleted > 0 {
        let sender = state.tx_pricefeed.clone();
        update_pricefeed(PriceFeedMessage::DeleteOrder(order_id), sender);
    }

    Json(deleted)
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending messages).
async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // We subscribe *before* sending the "joined" message, so that we will also
    // display it to our client.
    let mut rx = state.tx_pricefeed.subscribe();

    let mut conn = state.pool.clone().get().unwrap();
    let orders = orderbook::db::orders::all(&mut conn).unwrap();

    // Now send the "joined" message to all subscribers.
    let _ = state.tx_pricefeed.send(PriceFeedMessage::AllOrders(orders));

    // Spawn the first task that will receive broadcast messages and send text
    // messages over the websocket to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(st) = rx.recv().await {
            sender
                .send(Message::Text(serde_json::to_string(&st).unwrap()))
                .await
                .unwrap();
        }
    });

    // Clone things we want to pass (move) to the receiving task.
    let tx = state.tx_pricefeed.clone();

    // Spawn a task that takes messages from the websocket, prepends the user
    // name, and sends them to all broadcast subscribers.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let orders = serde_json::from_str(text.as_str()).unwrap();
            let _ = tx.send(orders);
        }
    });

    // If any one of the tasks run to completion, we abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}
