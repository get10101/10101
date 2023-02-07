use bdk::{
    bitcoin::{BlockHeader, Script, Transaction, Txid},
    blockchain::ElectrumBlockchain,
    electrum_client::{self, ElectrumApi},
    Error,
};

/// The height and confirmation status of a transaction
pub struct TxStatus {
    /// Whether the transaction has at least one confirmation
    pub confirmed: bool,
    /// The height of the block the transaction was confirmed in
    pub block_height: Option<u32>,
}

/// A trait Blockchains can implement if they support querying chain data
pub trait IndexedChain {
    /// Get the block header for a given block height
    fn get_header(&self, height: u32) -> Result<BlockHeader, Error>;

    /// Get the position of a specific transaction in a block
    fn get_position_in_block(&self, txid: &Txid, height: usize) -> Result<Option<usize>, Error>;

    /// Get the confirmation status and height of a transaction by Txid
    fn get_tx_status(&self, txid: &Txid) -> Result<Option<TxStatus>, Error>;

    /// Get all transactions that spend or fund a certain Script
    /// Includes the confirmation status and height for each transaction
    fn get_script_tx_history(&self, script: &Script)
        -> Result<Vec<(TxStatus, Transaction)>, Error>;
}

impl IndexedChain for ElectrumBlockchain {
    fn get_header(&self, height: u32) -> Result<BlockHeader, Error> {
        Ok(self.block_header(height as usize)?)
    }

    fn get_position_in_block(&self, txid: &Txid, height: usize) -> Result<Option<usize>, Error> {
        Ok(Some(self.transaction_get_merkle(txid, height)?.pos))
    }

    // TODO: This isn't great and I'm not even sure it works correctly
    //       it requires support for fetching `verbose` tx from electrum
    //       so that we can check for confirmations / confirmation height
    fn get_tx_status(&self, txid: &Txid) -> Result<Option<TxStatus>, Error> {
        match self.transaction_get(txid) {
            Ok(_tx) => Ok(Some(TxStatus {
                confirmed: true,
                block_height: None,
            })),
            Err(e) => match e {
                electrum_client::Error::Protocol(serde_json::Value::String(str))
                    if str.eq("missing transaction") =>
                {
                    Ok(None)
                }
                _ => Err(Error::Electrum(e)),
            },
        }
    }

    fn get_script_tx_history(
        &self,
        script: &Script,
    ) -> Result<Vec<(TxStatus, Transaction)>, Error> {
        let histories = self.script_get_history(script)?;

        let res: Result<Vec<(TxStatus, Transaction)>, Error> = histories
            .iter()
            .map(|history| {
                let status = {
                    if history.height <= 0 {
                        TxStatus {
                            confirmed: false,
                            block_height: None,
                        }
                    } else {
                        TxStatus {
                            confirmed: true,
                            block_height: Some(history.height as u32),
                        }
                    }
                };

                match self.transaction_get(&history.tx_hash) {
                    Ok(tx) => Ok((status, tx)),
                    Err(e) => Err(Error::Electrum(e)),
                }
            })
            .collect();

        res
    }
}
