use crate::commons::reqwest_client;
use crate::config;
use crate::db;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use ln_dlc_node::channel::ChannelState;
use ln_dlc_node::channel::UserChannelId;
use ln_dlc_node::node::rust_dlc_manager::subchannel::LNChannelManager;
use ln_dlc_node::node::rust_dlc_manager::ChannelId;
use ln_dlc_node::node::ChannelManager;
use parking_lot::Mutex;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Handle;
use crate::api::SendPayment;

const WAIT_FOR_OUTBOUND_CAPACITY_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct ChannelFeePaymentSubscriber {
    open_fee_amount: Arc<Mutex<Option<Decimal>>>,
    channel_manager: Arc<ChannelManager>,
}

impl Subscriber for ChannelFeePaymentSubscriber {
    fn notify(&self, event: &EventInternal) {
        if let EventInternal::PaymentClaimed(amount_msats) = event {
            if let Err(e) = self.pay_jit_channel_open_fee(*amount_msats) {
                tracing::error!("{e:#}");
            }
        }
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::PaymentClaimed]
    }
}

/// This subscriber tries to pay the channel opening fees through a regular lightning payment.
///
/// TODO(holzeis): This shouldn't be required once we implement a proper LSP flow for opening an
/// inbound channel to the user.
impl ChannelFeePaymentSubscriber {
    pub fn new(channel_manager: Arc<ChannelManager>) -> Self {
        Self {
            open_fee_amount: Arc::new(Mutex::new(None)),
            channel_manager,
        }
    }

    /// Attempts to pay the fees for opening an inbound channel.
    fn pay_jit_channel_open_fee(&self, amount_msats: u64) -> Result<()> {
        let channels = self.channel_manager.list_channels();
        // Assuming the user ever only has one channel. Needs to be changed when we are supporting
        // multiple open channels at the same time.
        let channel_details = channels.first();
        if channels.len() > 1 {
            let channel_id = channel_details
                .expect("expect channel detail to be some")
                .channel_id;
            tracing::warn!(
                channel_id = hex::encode(channel_id),
                "Found more than one channel! Using the first one"
            );
        }

        match channel_details {
            Some(channel_details) => {
                let user_channel_id = UserChannelId::from(channel_details.user_channel_id);
                let mut channel =
                    db::get_channel(&user_channel_id.to_string())?.with_context(|| {
                        format!("Couldn't find channel by user_channel_id {user_channel_id}")
                    })?;

                if channel.channel_state != ChannelState::OpenUnpaid {
                    tracing::debug!("Channel inbound fees have already been paid. Skipping.");
                    return Ok(());
                }

                let liquidity_option_id = match channel.liquidity_option_id {
                    Some(liquidity_option_id) => liquidity_option_id,
                    None => {
                        tracing::warn!("Couldn't find liquidity option. Not charging for the inbound channel creation.");
                        return Ok(());
                    }
                };

                let liquidity_options = tokio::task::block_in_place(ln_dlc::liquidity_options)?;
                let liquidity_option = liquidity_options
                    .iter()
                    .find(|l| l.id == liquidity_option_id)
                    .with_context(|| {
                        format!("Couldn't find liquidity option for id {liquidity_option_id}")
                    })?;

                let amount = Decimal::from(amount_msats) / Decimal::from(1000);
                let fee = match self.get_open_fee_amount() {
                    Some(fee) => fee,
                    None => {
                        let fee = liquidity_option.get_fee(amount);
                        self.set_open_fee_amount(fee);
                        fee
                    }
                };

                let fee_msats = fee.to_u64().expect("to fit into u64") * 1000;
                let channel_id = channel_details.channel_id;
                tokio::task::block_in_place(|| {
                    tracing::debug!(
                        %user_channel_id,
                        channel_id = hex::encode(channel_id),
                        fee_msats,
                        "Waiting for outbound capacity on channel to pay jit channel opening fee.",
                    );
                    Handle::current().block_on(async {
                        self.wait_for_outbound_capacity(channel_id, fee_msats)
                            .await?;
                        // We add another sleep to ensure that the channel has actually been updated
                        // after receiving the payment. Note, this is by no
                        // means ideal and should be revisited some other
                        // time.
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        anyhow::Ok(())
                    })
                })
                .context("Failed during wait for outbound capacity")?;

                tracing::debug!("Trying to pay channel opening fees of {fee} sats");
                let funding_txid = channel.funding_txid.with_context(|| format!("Funding transaction id for user_channel_id {user_channel_id} should be set after the ChannelReady event."))?;

                if fee > amount {
                    tracing::warn!("Trying to pay fees with an amount smaller than the fees!")
                }

                let invoice_str = tokio::task::block_in_place(|| {
                    Handle::current().block_on(fetch_fee_invoice(
                        fee.to_u32().expect("to fit into u32"),
                        funding_txid.to_string(),
                    ))
                })?;

                match ln_dlc::send_payment(SendPayment::Lightning {invoice: invoice_str, amount: None }) {
                    Ok(_) => {
                        // unset the open fee amount as the payment has been initiated.
                        self.unset_open_fee_amount();
                        channel.channel_state = ChannelState::Open;
                        db::upsert_channel(channel)?;
                        tracing::info!("Successfully triggered inbound channel fees payment of {fee} sats to {}", config::get_coordinator_info().pubkey);
                    }
                    Err(e) => {
                        tracing::error!("Failed to pay funding transaction fees of {fee} sats to {}. Error: {e:#}", config::get_coordinator_info().pubkey);
                    }
                };
            }
            None => tracing::warn!("Received a payment, but did not have any channel details"),
        }

        Ok(())
    }

    fn set_open_fee_amount(&self, fee: Decimal) {
        *self.open_fee_amount.lock() = Some(fee);
    }

    fn unset_open_fee_amount(&self) {
        *self.open_fee_amount.lock() = None;
    }

    fn get_open_fee_amount(&self) -> Option<Decimal> {
        *self.open_fee_amount.lock()
    }

    async fn wait_for_outbound_capacity(
        &self,
        channel_id: ChannelId,
        funding_tx_fees_msats: u64,
    ) -> Result<()> {
        tokio::time::timeout(WAIT_FOR_OUTBOUND_CAPACITY_TIMEOUT, async {
            loop {
                let channel_details = match self
                    .channel_manager
                    .get_channel_details(&channel_id) {
                    Some(channel_details) => channel_details,
                    None => {
                        bail!("Could not find channel details for {}", hex::encode(channel_id));
                    },
                };

                if channel_details.outbound_capacity_msat >= funding_tx_fees_msats {
                    tracing::debug!(channel_details.outbound_capacity_msat, channel_id=hex::encode(channel_id),
                        "Channel has enough outbound capacity");
                    return Ok(())
                } else {
                    tracing::debug!(channel_id = hex::encode(channel_id), outbound_capacity_msats = channel_details.outbound_capacity_msat, funding_tx_fees_msats,
                        "Channel does not have enough outbound capacity to pay jit channel opening fees yet. Waiting.");
                    tokio::time::sleep(Duration::from_millis(200)).await
                }
            }
        })
        .await?.map_err(|e| anyhow!("{e:#}"))
        .with_context(||format!(
            "Timed-out waiting for channel {} to become usable",
            hex::encode(channel_id)
        ))
    }
}

async fn fetch_fee_invoice(funding_tx_fee: u32, funding_txid: String) -> Result<String> {
    reqwest_client()
        .get(format!(
            "http://{}/api/invoice/open_channel_fee?amount={}&channel_funding_txid={}",
            config::get_http_endpoint(),
            funding_tx_fee,
            funding_txid.as_str()
        ))
        .send()
        .await?
        .text()
        .await
        .map_err(|e| anyhow!("Failed to fetch invoice from coordinator. Error:{e:?}"))
}
