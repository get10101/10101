use crate::node::Node;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use bdk::wallet::AddressIndex;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::Confirm;

#[derive(Debug, Clone)]
pub struct OffChainBalance {
    pub available: u64,
    pub pending_close: u64,
}

impl Node {
    pub fn sync(&self) {
        let confirmables = vec![
            &*self.channel_manager as &dyn Confirm,
            &*self.chain_monitor as &dyn Confirm,
        ];

        self.wallet.inner().sync(confirmables).unwrap();
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
}
