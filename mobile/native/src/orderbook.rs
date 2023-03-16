use crate::config;
use crate::trade::position;
use anyhow::Result;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::secp256k1::SECP256K1;
use futures::TryStreamExt;
use orderbook_commons::OrderbookMsg;
use orderbook_commons::Signature;
use state::Storage;
use std::time::Duration;
use tokio::runtime::Runtime;

const WS_RECONNECT_TIMEOUT_SECS: u64 = 2;

fn runtime() -> Result<&'static Runtime> {
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}

pub fn subscribe(secret_key: SecretKey) -> Result<()> {
    let runtime = runtime()?;

    runtime.spawn(async move {
        let url = format!(
            "ws://{}/api/orderbook/websocket",
            config::get_http_endpoint()
        );

        let pubkey = secret_key.public_key(SECP256K1);
        let authenticate = |msg| {
            let signature = secret_key.sign_ecdsa(msg);
            Signature { pubkey, signature }
        };

        loop {
            let mut stream =
                orderbook_client::subscribe_with_authentication(url.clone(), &authenticate);

            match stream.try_next().await {
                Ok(Some(result)) => {
                    tracing::debug!("Receive {result}");

                    let orderbook_message: OrderbookMsg =
                        match serde_json::from_str::<OrderbookMsg>(&result) {
                            Ok(message) => message,
                            Err(e) => {
                                tracing::error!(
                                    "Could not deserialize message from orderbook. Error: {e:#}"
                                );
                                continue;
                            }
                        };
                    match orderbook_message.clone() {
                        OrderbookMsg::Match(filled) => {
                            tracing::info!(
                                "Received a match from orderbook for order: {}",
                                filled.order_id
                            );

                            match position::handler::trade(filled).await {
                                Ok(_) => {
                                    tracing::info!("Successfully requested trade at coordinator")
                                }
                                Err(e) => tracing::error!(
                                    "Failed to request trade at coordinator. Error: {e:#}"
                                ),
                            }
                        }
                        _ => tracing::debug!(
                            "Skipping message from orderbook. {orderbook_message:?}"
                        ),
                    }
                }
                Ok(None) => {
                    tracing::warn!("Orderbook WS stream closed");
                }
                Err(error) => {
                    tracing::warn!(%error, "Orderbook WS stream closed with error");
                }
            }

            let timeout = Duration::from_secs(WS_RECONNECT_TIMEOUT_SECS);

            tracing::debug!(?timeout, "Reconnecting to orderbook WS after timeout");

            tokio::time::sleep(timeout).await;
        }
    });

    Ok(())
}
