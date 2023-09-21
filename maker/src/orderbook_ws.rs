use crate::position;
use crate::position::OrderTenTenOne;
use crate::position::PositionUpdateTenTenOne;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::SECP256K1;
use futures::FutureExt;
use futures::SinkExt;
use futures::TryStreamExt;
use orderbook_commons::FilledWith;
use orderbook_commons::Message;
use orderbook_commons::OrderbookRequest;
use reqwest::Url;
use std::time::Duration;
use tokio_tungstenite::tungstenite;

const RECONNECT_TIMEOUT: Duration = Duration::from_secs(2);

const REQUEST_FILLED_MATCHES_INTERVAL: Duration = Duration::from_secs(30);

/// Orderbook WebSocket client.
pub struct Client {
    /// Orderbook WebSocket URL.
    url: String,
    /// Trader ID of the maker.
    trader_id: PublicKey,
    /// Secret key used to authenticate against the orderbook.
    auth_sk: SecretKey,
    /// Where to forward position updates based on matched trades.
    position_manager: xtra::Address<position::Manager>,
}

impl Client {
    pub fn new(
        endpoint: Url,
        trader_id: PublicKey,
        auth_sk: SecretKey,
        position_manager: xtra::Address<position::Manager>,
    ) -> Self {
        let domain = endpoint.domain().expect("domain");
        let port = endpoint.port().expect("port");

        let url = format!("ws://{domain}:{port}/api/orderbook/websocket");

        Self {
            url,
            trader_id,
            auth_sk,
            position_manager,
        }
    }

    /// Spawn a task which subscribes to the orderbook's WebSocket API.
    ///
    /// The maker uses this to learn about the orders which resulted in a match.
    ///
    /// The task will attempt to reconnect to the WebSocket API if it encounters any errors.
    pub fn spawn_supervised_connection(&self) {
        let auth_sk = self.auth_sk;
        let trader_id = self.trader_id;
        let url = self.url.clone();
        let position_manager = self.position_manager.clone();

        tokio::spawn(async move {
            let auth_pk = auth_sk.public_key(SECP256K1);
            let auth_fn = move |msg| {
                let signature = auth_sk.sign_ecdsa(msg);
                orderbook_commons::Signature {
                    pubkey: auth_pk,
                    signature,
                }
            };

            loop {
                let url = url.clone();
                let authenticate = auth_fn;
                match orderbook_client::subscribe_with_authentication(url, authenticate).await {
                    Ok((mut sink, mut stream)) => {
                        // We request the filled matches for all our limit orders periodically.
                        let (task, _handle) = async move {
                            loop {
                                if let Err(e) = sink
                                    .send(
                                        tungstenite::Message::try_from(
                                            OrderbookRequest::LimitOrderFilledMatches { trader_id },
                                        )
                                        .expect("valid message"),
                                    )
                                    .await
                                {
                                    tracing::error!(
                                        "Failed to ask for limit order filled matches: {e:#}"
                                    );
                                };

                                tokio::time::sleep(REQUEST_FILLED_MATCHES_INTERVAL).await;
                            }
                        }
                        .remote_handle();

                        tokio::spawn(task);

                        while let Ok(Some(msg)) = stream.try_next().await {
                            if let Err(e) =
                                process_message(msg, &position_manager, &trader_id, &orderbook_status).await
                            {
                                tracing::error!("Failed to process orderbook message: {e:#}");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to connect to orderbook WS: {e:#}");
                    }
                }
                tokio::time::sleep(RECONNECT_TIMEOUT).await;
                tracing::debug!(
                    ?RECONNECT_TIMEOUT,
                    "Reconnecting to orderbook WS after timeout"
                );
            }
        });
    }
}

async fn process_message(
    msg: String,
    position_manager: &xtra::Address<position::Manager>,
    maker_trader_id: &PublicKey,
) -> Result<()> {
    tracing::trace!(%msg, "New message from orderbook");

    let msg = serde_json::from_str::<Message>(&msg).context("Deserialization failed")?;

    match msg {
        Message::LimitOrderFilledMatches { trader_id, matches } => {
            ensure!(
                trader_id == *maker_trader_id,
                "Got LimitOrderFilledMatches for wrong trader"
            );

            let orders = matches
                .into_iter()
                .map(|(order_id, contracts)| {
                    OrderTenTenOne::new(
                        order_id,
                        // TODO: Get `ContractSymbol` from the orderbook.
                        position::ContractSymbol::BtcUsd,
                        contracts,
                    )
                })
                .collect::<Vec<_>>();

            tracing::info!(n = %orders.len(), "Received limit order filled matches");

            let _ = position_manager.send(PositionUpdateTenTenOne(orders)).await;
        }
        Message::Match(FilledWith { order_id, .. }) => {
            // We cannot rely directly on this message because the match does not specify the
            // direction. We could use this as a trigger to ask the orderbook for all the relevant
            // information, but it's too early so the match won't even be filled.

            tracing::info!(%order_id, "Order matched");
        }
        Message::Authenticated => {
            tracing::info!("Orderbook authentication succeeded");
        }
        Message::InvalidAuthentication(e) => {
            tracing::error!("Orderbook authentication failed: {e}");
        }
        Message::AllOrders(_)
        | Message::NewOrder(_)
        | Message::DeleteOrder(_)
        | Message::Update(_)
        | Message::AsyncMatch { .. }
        | Message::Rollover => {
            // Nothing to do.
        }
    }

    Ok(())
}
