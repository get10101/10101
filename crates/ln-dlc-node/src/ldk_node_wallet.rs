use crate::bdk_actor::BdkActor;
use crate::bdk_actor::BuildAndSignTx;
use crate::bdk_actor::GetBalance;
use crate::bdk_actor::GetHistory;
use crate::bdk_actor::GetLastUnusedAddress;
use crate::bdk_actor::GetNewAddress;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln::TracingLogger;
use anyhow::Error;
use anyhow::Result;
use autometrics::autometrics;
use bdk::blockchain::Blockchain;
use bdk::blockchain::EsploraBlockchain;
use bdk::blockchain::GetHeight;
use bdk::TransactionDetails;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::BlockHash;
use bitcoin::Script;
use bitcoin::Transaction;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::transaction::OutPoint;
use lightning::chain::Filter;
use lightning::chain::WatchedOutput;
use lightning_transaction_sync::EsploraSyncClient;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Wallet {
    // A BDK blockchain used for wallet sync.
    pub(crate) blockchain: Arc<EsploraBlockchain>,
    settings: RwLock<WalletSettings>,
    esplora_sync_client: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
    fee_rate_estimator: Arc<FeeRateEstimator>,
    on_chain_actor: xtra::Address<BdkActor>,
}

#[derive(Clone, Debug, Default)]
pub struct WalletSettings {
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,
}

impl Wallet {
    pub(crate) fn new(
        blockchain: EsploraBlockchain,
        esplora_sync_client: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
        fee_rate_estimator: Arc<FeeRateEstimator>,
        on_chain_actor: xtra::Address<BdkActor>,
    ) -> Self {
        let settings = RwLock::new(WalletSettings::default());

        Self {
            blockchain: Arc::new(blockchain),
            settings,
            esplora_sync_client,
            fee_rate_estimator,
            on_chain_actor,
        }
    }

    pub async fn update_settings(&self, settings: WalletSettings) {
        tracing::info!(?settings, "Updating wallet settings");
        *self.settings.write().await = settings;
    }

    pub async fn settings(&self) -> WalletSettings {
        self.settings.read().await.clone()
    }

    /// Update the internal BDK wallet database with the blockchain.
    pub async fn sync(&self) -> Result<()> {
        self.on_chain_actor.send(crate::bdk_actor::Sync).await??;

        Ok(())
    }

    #[autometrics]
    pub(crate) async fn create_funding_transaction(
        &self,
        script_pubkey: Script,
        value_sats: u64,
        confirmation_target: ConfirmationTarget,
    ) -> Result<Transaction, Error> {
        let fee_rate = self.fee_rate_estimator.get(confirmation_target);
        let transaction = self
            .on_chain_actor
            .send(BuildAndSignTx {
                script_pubkey,
                amount_sats_or_drain: Some(value_sats),
                fee_rate,
            })
            .await??;

        Ok(transaction)
    }

    #[autometrics]
    pub(crate) async fn get_new_address(&self) -> Result<bitcoin::Address, Error> {
        let address = self.on_chain_actor.send(GetNewAddress).await??;

        Ok(address)
    }

    #[autometrics]
    pub async fn get_last_unused_address(&self) -> Result<bitcoin::Address, Error> {
        let address = self.on_chain_actor.send(GetLastUnusedAddress).await??;

        Ok(address)
    }

    #[autometrics]
    pub(crate) async fn get_balance(&self) -> Result<bdk::Balance, Error> {
        let balance = self.on_chain_actor.send(GetBalance).await??;

        Ok(balance)
    }

    #[autometrics]
    pub fn tip(&self) -> Result<(u32, BlockHash)> {
        let height = self.blockchain.get_height()?;
        let hash = self.blockchain.get_tip_hash()?;

        Ok((height, hash))
    }

    #[autometrics]
    pub async fn on_chain_transaction_list(&self) -> Result<Vec<TransactionDetails>> {
        let transactions = self.on_chain_actor.send(GetHistory).await??;

        Ok(transactions)
    }
}

impl BroadcasterInterface for Wallet {
    fn broadcast_transaction(&self, tx: &Transaction) {
        let txid = tx.txid();

        tracing::info!(%txid, raw_tx = %serialize_hex(&tx), "Broadcasting transaction");

        let txos = tx.output.clone();

        if let Err(err) = self.blockchain.broadcast(tx) {
            tracing::error!("Failed to broadcast transaction: {err:#}");
        }

        // FIXME: We've added this to ensure that we watch the outputs of any commitment transaction
        // we publish. This is incredibly hacky and probably doesn't scale, as we simply register
        // _every_ transaction output we ever publish. Obviously not all these outputs will be
        // spendable by us, so it might result in some weirdness, but it should be safe.
        //
        // Also, this doesn't cover the counterparty, which unfortunately is only able to find the
        // commitment transaction on-chain after a restart.
        for (i, output) in txos.into_iter().enumerate() {
            self.esplora_sync_client.register_output(WatchedOutput {
                block_hash: None,
                outpoint: OutPoint {
                    txid,
                    index: i as u16,
                },
                script_pubkey: output.script_pubkey,
            });
        }
    }
}
