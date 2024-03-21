use crate::bitcoin_conversion::to_secp_sk_30;
use crate::node::Node;
use crate::node::Storage;
use crate::on_chain_wallet;
use crate::on_chain_wallet::BdkStorage;
use crate::on_chain_wallet::OnChainWallet;
use crate::on_chain_wallet::TransactionDetails;
use crate::storage::TenTenOneStorage;
use anyhow::Context;
use anyhow::Result;
use bdk_esplora::EsploraAsyncExt;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::ScriptBuf;
use bitcoin::TxOut;
use lightning::chain::chaininterface::ConfirmationTarget;
use std::sync::Arc;
use tokio::task::spawn_blocking;

/// The number of parallel requests to be used during the on-chain sync.
///
/// This number was chosen arbitrarily.
const PARALLEL_REQUESTS_SYNC: usize = 5;

impl<D: BdkStorage, S: TenTenOneStorage, N: Storage + Send + Sync + 'static> Node<D, S, N> {
    pub fn wallet(&self) -> Arc<OnChainWallet<D>> {
        self.wallet.clone()
    }

    pub fn get_new_address(&self) -> Result<Address> {
        self.wallet.get_new_address()
    }

    pub fn get_unused_address(&self) -> Result<Address> {
        self.wallet.get_unused_address()
    }

    pub async fn get_blockchain_height(&self) -> Result<u64> {
        self.blockchain
            .get_blockchain_tip()
            .await
            .context("Failed to get blockchain height")
    }

    pub fn get_on_chain_balance(&self) -> bdk::wallet::Balance {
        self.wallet.get_balance()
    }

    pub fn node_key(&self) -> SecretKey {
        to_secp_sk_30(self.keys_manager.get_node_secret_key())
    }

    pub fn get_on_chain_history(&self) -> Vec<TransactionDetails> {
        self.wallet.get_on_chain_history()
    }

    pub fn get_utxos(&self) -> Vec<(OutPoint, TxOut)> {
        self.wallet.get_utxos()
    }

    pub fn is_mine(&self, script_pubkey: &ScriptBuf) -> bool {
        self.wallet.is_mine(script_pubkey)
    }

    /// Estimate the fee for sending the given `amount_sats` to the given `address` on-chain with
    /// the given `fee`.
    pub fn estimate_fee(
        &self,
        address: Address,
        amount_sats: u64,
        fee: ConfirmationTarget,
    ) -> Result<Amount, on_chain_wallet::EstimateFeeError> {
        self.wallet.estimate_fee(&address, amount_sats, fee)
    }

    /// Sync the state of the on-chain wallet against the blockchain.
    pub async fn sync_on_chain_wallet(&self) -> Result<()> {
        let client = &self.blockchain.esplora_client;

        let (local_chain, unused_revealed_script_pubkeys, unconfirmed_txids, utxos) =
            spawn_blocking({
                let wallet = self.wallet.clone();
                move || wallet.pre_sync_state()
            })
            .await
            .expect("task to complete");

        let graph_update = client
            .sync(
                unused_revealed_script_pubkeys,
                unconfirmed_txids,
                utxos,
                PARALLEL_REQUESTS_SYNC,
            )
            .await?;

        let chain_update = {
            let missing_heights = graph_update.missing_heights(&local_chain);

            client
                .update_local_chain(local_chain.tip(), missing_heights)
                .await?
        };

        let wallet_update = bdk::wallet::Update {
            graph: graph_update.clone(),
            chain: Some(chain_update),
            ..Default::default()
        };

        spawn_blocking({
            let wallet = self.wallet.clone();
            move || {
                wallet.commit_wallet_update(wallet_update)?;

                anyhow::Ok(())
            }
        })
        .await
        .expect("task to complete")?;

        Ok(())
    }

    pub async fn full_sync(&self, stop_gap: usize) -> Result<()> {
        let client = &self.blockchain.esplora_client;

        let (local_chain, all_script_pubkeys) = spawn_blocking({
            let wallet = self.wallet.clone();
            move || {
                let all_script_pubkeys = wallet.all_script_pubkeys();
                let local_chain = wallet.local_chain();

                (local_chain, all_script_pubkeys)
            }
        })
        .await
        .expect("task to complete");

        let (graph_update, last_active_indices) = client
            .full_scan(all_script_pubkeys, stop_gap, PARALLEL_REQUESTS_SYNC)
            .await?;

        let chain_update = {
            let missing_heights = graph_update.missing_heights(&local_chain);

            client
                .update_local_chain(local_chain.tip(), missing_heights)
                .await?
        };

        let wallet_update = bdk::wallet::Update {
            graph: graph_update.clone(),
            chain: Some(chain_update),
            last_active_indices,
        };

        spawn_blocking({
            let wallet = self.wallet.clone();
            move || {
                wallet.commit_wallet_update(wallet_update)?;

                anyhow::Ok(())
            }
        })
        .await
        .expect("task to complete")?;

        Ok(())
    }
}
