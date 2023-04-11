use crate::node::HTLCStatus;
use crate::node::Node;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::wallet::AddressIndex;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::Confirm;
use lightning::ln::PaymentHash;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct OffChainBalance {
    pub available: u64,
    pub pending_close: u64,
}

impl Node {
    pub fn get_seed_phrase(&self) -> Vec<String> {
        self.wallet.get_seed_phrase()
    }

    pub fn sync(&self) -> Result<()> {
        let confirmables = vec![
            &*self.channel_manager as &dyn Confirm,
            &*self.chain_monitor as &dyn Confirm,
        ];

        self.wallet
            .inner()
            .sync(confirmables)
            .map_err(|e| anyhow!("{e:#}"))
    }

    pub fn get_new_address(&self) -> Result<Address> {
        let address = self
            .wallet
            .inner()
            .get_wallet()
            .unwrap()
            .get_address(AddressIndex::New)?;

        Ok(address.address)
    }

    pub fn get_on_chain_balance(&self) -> Result<bdk::Balance> {
        self.wallet.inner().get_balance().map_err(|e| anyhow!(e))
    }

    pub fn node_key(&self) -> Result<SecretKey> {
        match self.keys_manager.get_node_secret(Recipient::Node) {
            Ok(key) => Ok(key),
            Err(()) => {
                bail!("Could not get secret key from node")
            }
        }
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
            tracing::trace!("Pending on-chain balance from channel closure: {balance:?}");

            use ::lightning::chain::channelmonitor::Balance::*;
            match balance {
                ClaimableOnChannelClose {
                    claimable_amount_satoshis,
                }
                | ClaimableAwaitingConfirmations {
                    claimable_amount_satoshis,
                    ..
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
            }
        });

        let available = self
            .channel_manager
            .list_channels()
            .iter()
            .map(|details| details.balance_msat / 1000)
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

    pub fn get_off_chain_history(&self) -> Vec<PaymentDetails> {
        let inbound_payments = self
            .inbound_payments
            .lock()
            .expect("to be able to acquire lock");
        let inbound_payments = inbound_payments.iter().map(|(hash, info)| PaymentDetails {
            payment_hash: *hash,
            status: info.status,
            flow: PaymentFlow::Inbound,
            amount_msat: info.amt_msat.0,
            timestamp: info.timestamp,
        });

        let outbound_payments = self
            .outbound_payments
            .lock()
            .expect("to be able to acquire lock");
        let outbound_payments = outbound_payments.iter().map(|(hash, info)| PaymentDetails {
            payment_hash: *hash,
            status: info.status,
            flow: PaymentFlow::Outbound,
            amount_msat: info.amt_msat.0,
            timestamp: info.timestamp,
        });

        let mut payments = inbound_payments
            .chain(outbound_payments)
            .collect::<Vec<_>>();

        payments.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        payments
    }
}

pub struct PaymentDetails {
    pub payment_hash: PaymentHash,
    pub status: HTLCStatus,
    pub flow: PaymentFlow,
    pub amount_msat: Option<u64>,
    pub timestamp: OffsetDateTime,
}

pub enum PaymentFlow {
    Inbound,
    Outbound,
}
