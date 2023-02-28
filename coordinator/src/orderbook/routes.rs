use crate::orderbook::models::Order;
use crate::routes::AppState;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::Path;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use futures::SinkExt;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

pub async fn get_orders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let order = Order::all(&mut conn).unwrap();

    Json(order)
}

pub async fn get_order(
    Path(order_id): Path<i32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let pool = state.pool.clone();
    let mut conn = pool.get().unwrap();
    let order = Order::get_with_id(&mut conn, order_id).unwrap().unwrap();

    Json(order)
}

#[derive(Deserialize, Serialize)]
pub struct NewOrder {
    pub price: i32,
    pub maker_id: String,
    pub taken: bool,
}

impl From<NewOrder> for crate::orderbook::models::NewOrder {
    fn from(value: NewOrder) -> Self {
        crate::orderbook::models::NewOrder {
            price: value.price,
            maker_id: value.maker_id,
            taken: value.taken,
        }
    }
}

pub async fn post_order(
    State(state): State<Arc<AppState>>,
    Json(new_order): Json<NewOrder>,
) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let inserted = Order::insert(&mut conn, new_order.into()).unwrap();

    let sender = state.tx_pricefeed.clone();
    update_pricefeed(PriceFeedMessage::NewOrder(inserted.clone()), sender);

    Json(inserted)
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
    let order = Order::update(&mut conn, order_id, updated_order.taken).unwrap();
    let sender = state.tx_pricefeed.clone();
    update_pricefeed(PriceFeedMessage::Update(order.clone()), sender);

    Json(order)
}

pub async fn delete_order(
    Path(order_id): Path<i32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut conn = state.pool.clone().get().unwrap();
    let deleted = Order::delete_with_id(&mut conn, order_id).unwrap();
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
    let orders = Order::all(&mut conn).unwrap();

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
