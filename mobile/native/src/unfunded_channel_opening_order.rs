use crate::event;
use crate::event::EventInternal;
use crate::event::FundingChannelTask;
use crate::hodl_invoice;
use crate::state::get_node;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use crate::watcher;
use anyhow::Error;
use bitcoin::Amount;
use futures::FutureExt;
use xxi_node::commons::ChannelOpeningParams;

pub struct ExternalFunding {
    pub bitcoin_address: String,
    pub payment_request: String,
}

/// handles orders which would open a channel where the user does not have funds in his wallets
/// prior to the call
///
/// there are two things that are happening here:
/// 1. we watch an on-chain address of funding arrives
/// 2. we ask the coordinator for a hodl invoice and watch for it getting paid
///
/// if either 1) or 2) of those two tusk report that the funds are here, we continue and post the
/// order. if task 2) was done (hodl invoice), we also share the pre-image with the coordinator
pub async fn unfunded_channel_opening_order(
    order: NewOrder,
    coordinator_reserve: u64,
    trader_reserve: u64,
    estimated_margin: u64,
) -> anyhow::Result<anyhow::Result<ExternalFunding, Error>, Error> {
    let node = get_node();
    let bitcoin_address = node.inner.get_new_address()?;
    let funding_amount = Amount::from_sat(estimated_margin + trader_reserve);
    let hodl_invoice = hodl_invoice::get_hodl_invoice_from_coordinator(funding_amount).await?;

    let runtime = crate::state::get_or_create_tokio_runtime()?;
    let (future, remote_handle) = runtime.spawn({
        let bitcoin_address = bitcoin_address.clone();
        async move {
            event::publish(&EventInternal::FundingChannelNotification(
                FundingChannelTask::Pending,
            ));

            // we must only create the order on either event. If the bitcoin address is funded we cancel the watch for the lightning invoice and vice versa.
            tokio::select! {
                _ = watcher::watch_funding_address(bitcoin_address.clone(), funding_amount) => {
                    // received bitcoin payment.
                    tracing::info!(%bitcoin_address, %funding_amount, "Found funding amount on bitcoin address.")
                }
                _ = watcher::watch_lightning_payment() => {
                    // received lightning payment.
                    tracing::info!(%funding_amount, "Found lighting payment.")
                }
            }

            event::publish(&EventInternal::FundingChannelNotification(
                FundingChannelTask::Funded,
            ));

            tracing::debug!(
                coordinator_reserve,
                %funding_amount,
                "Creating new order with values {order:?}"
            );

            match order::handler::submit_order(
                order.into(),
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
            };
        }
    }).remote_handle();

    // We need to store the handle which will drop any old handler if present.
    node.watcher_handle.lock().replace(remote_handle);

    // Only now we can spawn the future, as otherwise we might have two competing handlers
    runtime.spawn(future);

    Ok(Ok(ExternalFunding {
        bitcoin_address: bitcoin_address.to_string(),
        payment_request: hodl_invoice.payment_request,
    }))
}
