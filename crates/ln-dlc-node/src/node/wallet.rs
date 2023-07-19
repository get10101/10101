use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ldk_node_wallet;
use crate::node::HTLCStatus;
use crate::node::Node;
use crate::node::Storage;
use crate::PaymentFlow;
use anyhow::Context;
use anyhow::Result;
use bdk::blockchain::EsploraBlockchain;
use bdk::sled;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use lightning::ln::PaymentHash;
use std::sync::Arc;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct OffChainBalance {
    /// Available balance, in msats.
    available: u64,
    /// Balance corresponding to channels being closed, in _sats_.
    pending_close: u64,
}

impl OffChainBalance {
    // TODO: We might want to reconsider how we convert from msats to sats.

    /// Available balance, in sats.
    pub fn available(&self) -> u64 {
        self.available / 1000
    }

    /// Balance corresponding to channels being closed, in sats.
    pub fn pending_close(&self) -> u64 {
        self.pending_close
    }

    /// Available balance, in msats.
    pub fn available_msat(&self) -> u64 {
        self.available
    }
}

impl<P> Node<P>
where
    P: Storage,
{
    pub fn get_seed_phrase(&self) -> Vec<String> {
        self.wallet.get_seed_phrase()
    }

    pub fn wallet(
        &self,
    ) -> Arc<ldk_node_wallet::Wallet<sled::Tree, EsploraBlockchain, FeeRateEstimator>> {
        self.wallet.inner()
    }

    pub fn get_unused_address(&self) -> Address {
        self.wallet.unused_address()
    }

    pub fn get_on_chain_balance(&self) -> Result<bdk::Balance> {
        self.wallet
            .inner()
            .get_balance()
            .context("Failed to get on-chain balance")
    }

    pub fn node_key(&self) -> SecretKey {
        self.keys_manager.get_node_secret_key()
    }

    /// The LDK [`OffChain`] balance keeps track of:
    ///
    /// - The total sum of money in all open channels.
    /// - The total sum of money in close transactions that do not yet pay to our on-chain wallet.
    pub fn get_ldk_balance(&self) -> OffChainBalance {
        let open_channels = self.channel_manager.list_channels();

        let claimable_channel_balances = {
            let ignored_channels = open_channels.iter().collect::<Vec<_>>();
            let ignored_channels = &ignored_channels.as_slice();
            self.chain_monitor.get_claimable_balances(ignored_channels)
        };

        let pending_close = claimable_channel_balances.iter().fold(0, |acc, balance| {
            tracing::debug!("Pending on-chain balance from channel closure: {balance:?}");

            use ::lightning::chain::channelmonitor::Balance::*;
            match balance {
                ClaimableOnChannelClose {
                    claimable_amount_satoshis,
                }
                | ContentiousClaimable {
                    claimable_amount_satoshis,
                    ..
                }
                | MaybeTimeoutClaimableHTLC {
                    claimable_amount_satoshis,
                    ..
                }
                | MaybePreimageClaimableHTLC {
                    claimable_amount_satoshis,
                    ..
                }
                | CounterpartyRevokedOutputClaimable {
                    claimable_amount_satoshis,
                } => acc + claimable_amount_satoshis,
                ClaimableAwaitingConfirmations { .. } => {
                    // we can safely ignore this type of balance because we override the
                    // `destination_script` for the channel closure so that it's owned by our
                    // on-chain wallet
                    acc
                }
            }
        });

        let available = self
            .channel_manager
            .list_channels()
            .iter()
            .map(|details| details.balance_msat)
            .sum();

        OffChainBalance {
            available,
            pending_close,
        }
    }

    pub fn get_on_chain_history(&self) -> Result<Vec<bdk::TransactionDetails>> {
        self.wallet
            .on_chain_transactions()
            .context("Failed to retrieve on-chain transaction history")
    }

    pub fn get_off_chain_history(&self) -> Result<Vec<PaymentDetails>> {
        let mut payments = self
            .storage
            .all_payments()?
            .iter()
            .map(|(hash, info)| PaymentDetails {
                payment_hash: *hash,
                status: info.status,
                flow: info.flow,
                amount_msat: info.amt_msat.0,
                timestamp: info.timestamp,
                description: info.description.clone(),
            })
            .collect::<Vec<_>>();

        payments.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(payments)
    }
}

#[derive(Debug)]
pub struct PaymentDetails {
    pub payment_hash: PaymentHash,
    pub status: HTLCStatus,
    pub flow: PaymentFlow,
    pub amount_msat: Option<u64>,
    pub timestamp: OffsetDateTime,
    pub description: String,
}
