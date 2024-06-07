use crate::check_version::check_version;
use crate::db;
use crate::message::TraderMessage;
use crate::node::Node;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::trade::TradeExecutor;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::Network;
use futures::future::RemoteHandle;
use futures::FutureExt;
use rust_decimal::prelude::ToPrimitive;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use xxi_node::commons;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::FilledWith;
use xxi_node::commons::Match;
use xxi_node::commons::Matches;
use xxi_node::commons::OrderState;
use xxi_node::commons::TradeAndChannelParams;
use xxi_node::commons::TradeParams;
use xxi_node::node::event::NodeEvent;

pub fn monitor(
    node: Node,
    mut receiver: broadcast::Receiver<NodeEvent>,
    notifier: mpsc::Sender<TraderMessage>,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> RemoteHandle<()> {
    let (fut, remote_handle) = async move {
        loop {
            match receiver.recv().await {
                Ok(NodeEvent::Connected { peer: trader_id }) => {
                    tokio::spawn({
                        let notifier = notifier.clone();
                        let node = node.clone();
                        async move {
                            tracing::debug!(
                                %trader_id,
                                "Checking if the user needs to be notified about pending matches"
                            );
                            if let Err(e) =
                                process_pending_match(node, notifier, trader_id, network, oracle_pk)
                                    .await
                            {
                                tracing::error!("Failed to process pending match. Error: {e:#}");
                            }
                        }
                    });
                }
                Ok(_) => {} // ignoring other node events
                Err(RecvError::Closed) => {
                    tracing::error!("Node event sender died! Channel closed.");
                    break;
                }
                Err(RecvError::Lagged(skip)) => {
                    tracing::warn!(%skip, "Lagging behind on node events.")
                }
            }
        }
    }
    .remote_handle();

    tokio::spawn(fut);

    remote_handle
}

/// Checks if there are any pending matches
async fn process_pending_match(
    node: Node,
    notifier: mpsc::Sender<TraderMessage>,
    trader_id: PublicKey,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> Result<()> {
    let mut conn = spawn_blocking({
        let node = node.clone();
        move || node.pool.get()
    })
    .await
    .expect("task to complete")?;

    if check_version(&mut conn, &trader_id).is_err() {
        tracing::info!(%trader_id, "User is not on the latest version. Skipping check if user needs to be informed about pending matches.");
        return Ok(());
    }

    if let Some(order) =
        orders::get_by_trader_id_and_state(&mut conn, trader_id, OrderState::Matched)?
    {
        tracing::debug!(%trader_id, order_id=%order.id, "Executing pending match");

        let matches = matches::get_matches_by_order_id(&mut conn, order.id)?;
        let filled_with = get_filled_with_from_matches(matches, network, oracle_pk)?;

        let channel_opening_params =
            db::channel_opening_params::get_by_order_id(&mut conn, order.id)?;

        tracing::info!(trader_id = %order.trader_id, order_id = %order.id, order_reason = ?order.order_reason, "Executing trade for match");
        let trade_executor = TradeExecutor::new(node, notifier);
        trade_executor
            .execute(&TradeAndChannelParams {
                trade_params: TradeParams {
                    pubkey: trader_id,
                    contract_symbol: ContractSymbol::BtcUsd,
                    leverage: order.leverage,
                    quantity: order.quantity.to_f32().expect("to fit into f32"),
                    direction: order.direction,
                    filled_with,
                },
                trader_reserve: channel_opening_params.map(|c| c.trader_reserve),
                coordinator_reserve: channel_opening_params.map(|c| c.coordinator_reserve),
                external_funding: channel_opening_params.and_then(|c| c.external_funding),
            })
            .await;
    }

    Ok(())
}

fn get_filled_with_from_matches(
    matches: Vec<Matches>,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> Result<FilledWith> {
    ensure!(
        !matches.is_empty(),
        "Need at least one matches record to construct a FilledWith"
    );

    let order_id = matches
        .first()
        .expect("to have at least one match")
        .order_id;

    let expiry_timestamp = commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);

    Ok(FilledWith {
        order_id,
        expiry_timestamp,
        oracle_pk,
        matches: matches
            .iter()
            .map(|m| Match {
                id: m.id,
                order_id: m.order_id,
                quantity: m.quantity,
                pubkey: m.match_trader_id,
                execution_price: m.execution_price,
                matching_fee: m.matching_fee,
            })
            .collect(),
    })
}
