use crate::bitcoin_conversion::to_tx_30;
use crate::node::Storage;
use anyhow::Context;
use anyhow::Result;
use bdk_esplora::esplora_client;
use bdk_esplora::esplora_client::OutputStatus;
use bdk_esplora::esplora_client::TxStatus;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::Block;
use bitcoin::BlockHash;
use bitcoin::OutPoint;
use bitcoin::Transaction;
use bitcoin::Txid;
use futures::executor::block_on;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::instrument;

const SOCKET_TIMEOUT: u64 = 30;

#[derive(Clone)]
pub struct Blockchain<N> {
    /// Async client used during on-chain syncing and, sometimes, to broadcast transactions.
    pub(crate) esplora_client: esplora_client::AsyncClient,
    node_storage: Arc<N>,
}

impl<N> Blockchain<N>
where
    N: Storage + Send + Sync,
{
    pub fn new(electrs_url: String, node_storage: Arc<N>) -> Result<Self> {
        let esplora_client_async = esplora_client::Builder::new(&electrs_url)
            .timeout(SOCKET_TIMEOUT)
            .build_async()?;

        Ok(Self {
            esplora_client: esplora_client_async,
            node_storage,
        })
    }

    pub async fn get_blockchain_tip(&self) -> Result<u64> {
        let height = self.esplora_client.get_height().await?;

        Ok(height as u64)
    }

    pub async fn get_block_hash(&self, height: u64) -> Result<BlockHash> {
        let block_hash = self.esplora_client.get_block_hash(height as u32).await?;

        Ok(block_hash)
    }

    pub async fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Block> {
        let block = self
            .esplora_client
            .get_block_by_hash(block_hash)
            .await?
            .context("Could not find block")?;

        Ok(block)
    }

    pub async fn get_transaction(&self, txid: &Txid) -> Result<Option<Transaction>> {
        let tx = self.esplora_client.get_tx(txid).await?;

        Ok(tx)
    }

    pub async fn get_transaction_confirmations(&self, txid: &Txid) -> Result<u32> {
        let status = self.esplora_client.get_tx_status(txid).await?;

        let tx_height = match status.block_height {
            Some(height) => height,
            None => return Ok(0),
        };

        self.tx_height_to_confirmations(tx_height).await
    }

    pub async fn get_txo_confirmations(&self, txo: &OutPoint) -> Result<Option<(u32, Txid)>> {
        let status = self
            .esplora_client
            .get_output_status(&txo.txid, txo.vout as u64)
            .await?;

        let (tx_height, txid) = match status {
            Some(OutputStatus {
                txid: Some(txid),
                status:
                    Some(TxStatus {
                        block_height: Some(height),
                        ..
                    }),
                ..
            }) => (height, txid),
            _ => return Ok(None),
        };

        let confirmations = self.tx_height_to_confirmations(tx_height).await?;

        Ok(Some((confirmations, txid)))
    }

    async fn tx_height_to_confirmations(&self, tx_height: u32) -> Result<u32> {
        let tip = self.esplora_client.get_height().await?;

        let confirmations = match tip.checked_sub(tx_height) {
            Some(diff) => diff + 1,
            // Something is wrong if the tip is behind the transaction confirmation height. We
            // simply mark the transaction as not confirmed.
            None => return Ok(0),
        };

        Ok(confirmations)
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

        self.esplora_client
            .broadcast(tx)
            .await
            .with_context(|| format!("Failed to broadcast transaction {txid}"))?;

        Ok(txid)
    }
}

impl<N> lightning::chain::chaininterface::BroadcasterInterface for Blockchain<N>
where
    N: Storage + Send + Sync + 'static,
{
    fn broadcast_transactions(&self, txs: &[&bitcoin_old::Transaction]) {
        for tx in txs {
            let tx = to_tx_30((*tx).clone());

            if let Err(e) = block_on(self.broadcast_transaction(&tx)) {
                tracing::error!(tx = %tx.txid(), "{e:#}");
            }
        }
    }
}
