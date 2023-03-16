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

            loop {
                match stream.try_next().await {
                    Ok(Some(msg)) => {
                        tracing::debug!(%msg, "New message from orderbook");

                        let msg = match serde_json::from_str::<OrderbookMsg>(&msg) {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::error!(
                                    "Could not deserialize message from orderbook. Error: {e:#}"
                                );
                                continue;
                            }
                        };

                        match msg {
                            OrderbookMsg::Match(filled) => {
                                tracing::info!(order_id = %filled.order_id, "Received match from orderbook");

                                if let Err(e) = position::handler::trade(filled).await {
                                    tracing::error!("Trade request sent to coordinator failed. Error: {e:#}");
                                }
                            },
                            _ => tracing::debug!(?msg, "Skipping message from orderbook"),
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("Orderbook WS stream closed");
                        break;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Orderbook WS stream closed with error");
                        break;
                    }
                }
            };

            let timeout = Duration::from_secs(WS_RECONNECT_TIMEOUT_SECS);

            tracing::debug!(?timeout, "Reconnecting to orderbook WS after timeout");

            tokio::time::sleep(timeout).await;
        }
    });

    Ok(())
}
