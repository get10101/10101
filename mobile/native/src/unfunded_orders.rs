use crate::event;
use crate::event::EventInternal;
use crate::event::FundingChannelTask;
use crate::state;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use futures::FutureExt;
use std::time::Duration;
use xxi_node::commons::ChannelOpeningParams;

pub(crate) async fn submit_unfunded_wallet_channel_opening_order(
    funding_address: Address,
    new_order: NewOrder,
    coordinator_reserve: u64,
    trader_reserve: u64,
    needed_channel_size: u64,
) -> Result<()> {
    let node = state::get_node().clone();
    let bdk_node = node.inner.clone();
    event::publish(&EventInternal::FundingChannelNotification(
        FundingChannelTask::Pending,
    ));
    let runtime = crate::state::get_or_create_tokio_runtime()?;
    let (future, remote_handle) = async move {
        loop {
            match bdk_node.get_unspent_txs(&funding_address).await {
                Ok(ref v) if v.is_empty() => {
                    tracing::debug!(
                        address = funding_address.to_string(),
                        amount = needed_channel_size.to_string(),
                        "No tx found for address"
                    );
                }
                Ok(txs) => {
                    // we sum up the total value in this output and check if it is big enough
                    // for the order
                    let total_unspent_amount_received = txs
                        .into_iter()
                        .map(|(_, amount)| amount.to_sat())
                        .sum::<u64>();

                    if total_unspent_amount_received >= needed_channel_size {
                        tracing::info!(
                            amount = total_unspent_amount_received.to_string(),
                            address = funding_address.to_string(),
                            "Address has been funded enough"
                        );
                        break;
                    }
                    tracing::debug!(
                        amount = total_unspent_amount_received.to_string(),
                        address = funding_address.to_string(),
                        "Address has not enough funds yet"
                    );
                }
                Err(err) => {
                    tracing::error!("Could not get utxo for address {err:?}")
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        event::publish(&EventInternal::FundingChannelNotification(
            FundingChannelTask::Funded,
        ));

        if let Err(error) = bdk_node.sync_on_chain_wallet().await {
            tracing::error!("Failed at syncing wallet {error:?}")
        }

        let balance = bdk_node.get_on_chain_balance();
        tracing::debug!(balance = balance.to_string(), "Wallet synced");

        tracing::debug!(
            coordinator_reserve,
            needed_channel_size,
            "Creating new order with values {new_order:?}"
        );

        match order::handler::submit_order(
            new_order.into(),
            Some(ChannelOpeningParams {
                coordinator_reserve: Amount::from_sat(coordinator_reserve),
                trader_reserve: Amount::from_sat(trader_reserve),
            }),
        )
        .await
        .map_err(anyhow::Error::new)
        .map(|id| id.to_string())
        {
            Ok(order_id) => {
                tracing::info!(order_id, "Order created");
                event::publish(&EventInternal::FundingChannelNotification(
                    FundingChannelTask::OrderCreated(order_id),
                ));
            }
            Err(error) => {
                tracing::error!("Failed at submitting order {error:?}");
                event::publish(&EventInternal::FundingChannelNotification(
                    FundingChannelTask::Failed("Failed at posting the order".to_string()),
                ));
            }
        }
    }
    .remote_handle();

    // We need to store the handle which will drop any old handler if present.
    node.unfunded_order_handle.lock().replace(remote_handle);

    // Only now we can spawn the future, as otherwise we might have two competing handlers
    runtime.spawn(future);

    Ok(())
}
