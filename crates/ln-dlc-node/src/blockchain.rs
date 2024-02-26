use crate::bitcoin_conversion::to_tx_30;
use crate::node::Storage;
use anyhow::Context;
use anyhow::Result;
use bdk_esplora::esplora_client;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::Block;
use bitcoin::BlockHash;
use bitcoin::Transaction;
use bitcoin::Txid;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::instrument;

const SOCKET_TIMEOUT: u64 = 30;

#[derive(Clone)]
pub struct Blockchain<N> {
    /// Async client used during on-chain syncing and, sometimes, to broadcast transactions.
    pub(crate) esplora_client_async: esplora_client::AsyncClient,
    /// Blocking client used when the task to be performed is in a blocking context (usually
    /// blocking trait methods).
    esplora_client_blocking: esplora_client::BlockingClient,
    node_storage: Arc<N>,
}

impl<N> Blockchain<N>
where
    N: Storage,
{
    pub fn new(electrs_url: String, node_storage: Arc<N>) -> Result<Self> {
        let esplora_client_async = esplora_client::Builder::new(&electrs_url)
            .timeout(SOCKET_TIMEOUT)
            .build_async()?;
        let esplora_client_blocking = esplora_client::Builder::new(&electrs_url)
            .timeout(SOCKET_TIMEOUT)
            .build_blocking()?;

        Ok(Self {
            esplora_client_async,
            esplora_client_blocking,
            node_storage,
        })
    }

    #[instrument(skip_all, fields(txid = %tx.txid()))]
    pub fn broadcast_transaction_blocking(&self, tx: &Transaction) -> Result<Txid> {
        let txid = tx.txid();

        tracing::info!(raw_tx = %serialize_hex(&tx), "Broadcasting transaction");

        if let Err(e) = self.node_storage.upsert_transaction(tx.into()) {
            tracing::error!("Failed to store transaction. Error: {e:#}");
        }

        self.esplora_client_blocking
            .broadcast(tx)
            .with_context(|| format!("Failed to broadcast transaction {txid}"))?;

        Ok(txid)
    }

    pub fn get_blockchain_tip(&self) -> Result<u64> {
        let height = self.esplora_client_blocking.get_height()?;

        Ok(height as u64)
    }

    pub fn get_block_hash(&self, height: u64) -> Result<BlockHash> {
        let block_hash = self.esplora_client_blocking.get_block_hash(height as u32)?;

        Ok(block_hash)
    }

    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Block> {
        let block = self
            .esplora_client_blocking
            .get_block_by_hash(block_hash)?
            .context("Could not find block")?;

        Ok(block)
    }
}

impl<N> Blockchain<N>
where
    N: Storage + Send + Sync + 'static,
{
    #[instrument(skip_all, fields(txid = %tx.txid()))]
    pub async fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid> {
        let txid = tx.txid();

        tracing::info!(raw_tx = %serialize_hex(&tx), "Broadcasting transaction");

        if let Err(e) = spawn_blocking({
            let storage = self.node_storage.clone();
            let tx = tx.into();
            move || {
                storage.upsert_transaction(tx)?;
                anyhow::Ok(())
            }
        })
        .await
        .expect("task to complete")
        {
            tracing::error!("Failed to store transaction. Error: {e:#}");
        }

        self.esplora_client_async
            .broadcast(tx)
            .await
            .with_context(|| format!("Failed to broadcast transaction {txid}"))?;

        Ok(txid)
    }
}

impl<N> lightning::chain::chaininterface::BroadcasterInterface for Blockchain<N>
where
    N: Storage,
{
    fn broadcast_transactions(&self, txs: &[&bitcoin_old::Transaction]) {
        for tx in txs {
            let tx = to_tx_30((*tx).clone());

            if let Err(e) = self.broadcast_transaction_blocking(&tx) {
                tracing::error!(tx = %tx.txid(), "{e:#}");
            }
        }
    }
}
