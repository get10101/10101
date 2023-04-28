use bdk::bitcoin::BlockHeader;
use bdk::bitcoin::Script;
use bdk::bitcoin::Transaction;
use bdk::bitcoin::Txid;
use bdk::blockchain::EsploraBlockchain;
use bdk::esplora_client;
use bdk::Error;
use async_trait::async_trait;

/// The height and confirmation status of a transaction
pub struct TxStatus {
    /// Whether the transaction has at least one confirmation
    pub confirmed: bool,
    /// The height of the block the transaction was confirmed in
    pub block_height: Option<u32>,
}

/// A trait Blockchains can implement if they support querying chain data
#[async_trait]
pub trait IndexedChain {
    /// Get the block header for a given block height
    async fn get_header(&self, height: u32) -> Result<BlockHeader, Error>;

    /// Get the position of a specific transaction in a block
    async fn get_position_in_block(&self, txid: &Txid, height: usize) -> Result<Option<usize>, Error>;

    /// Get the confirmation status and height of a transaction by Txid
    async fn get_tx_status(&self, txid: &Txid) -> Result<Option<TxStatus>, Error>;

    /// Get all transactions that spend or fund a certain Script
    /// Includes the confirmation status and height for each transaction
    async fn get_script_tx_history(&self, script: &Script)
        -> Result<Vec<(TxStatus, Transaction)>, Error>;
}

impl IndexedChain for EsploraBlockchain {
    async fn get_header(&self, height: u32) -> Result<BlockHeader, Error> {
        todo!("Figure out how to do this")
    }

    async fn get_position_in_block(&self, txid: &Txid, height: usize) -> Result<Option<usize>, Error> {
        let proof = self.get_merkle_proof(txid).await.map_err(|e| Error::Esplora(Box::new(e)))?.map(|merkle_proof| merkle_proof.pos);
        Ok(proof)
    }

    // TODO: This isn't great and I'm not even sure it works correctly
    //       it requires support for fetching `verbose` tx from electrum
    //       so that we can check for confirmations / confirmation height
    async fn get_tx_status(&self, txid: &Txid) -> Result<Option<TxStatus>, Error> {
        match self.get_tx(txid).await {
            Ok(_tx) => Ok(Some(TxStatus {
                confirmed: true,
                block_height: None,
            })),
            Err(e) => match e {
                esplora_client::Error::TransactionNotFound(_) =>
                {
                    Ok(None)
                }
                _ => Err(Error::Esplora(Box::new(e))),
            },
        }
    }

    async fn get_script_tx_history(
        &self,
        script: &Script,
    ) -> Result<Vec<(TxStatus, Transaction)>, Error> {
        let histories = self.scripthash_txs(script, None).await?;

        let res: Result<Vec<(TxStatus, Transaction)>, Error> = histories
            .into_iter()
            .map(|history| {
                let status = history.status;

                match self.get_tx(&history.tx_hash) {
                    Ok(tx) => Ok((status, tx)),
                    Err(e) => Err(Error::Esplora(Box::new(e))),
                }
            })
            .collect();

        res
    }
}
