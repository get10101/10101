use bdk::blockchain::ElectrumBlockchain;
use bdk::sled;
use bdk::wallet::AddressIndex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::{Address, BlockHash, BlockHeader};
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::TxOut;
use dlc_manager::error::Error;
use dlc_manager::error::Error::WalletError;
use dlc_manager::Signer;
use dlc_manager::Utxo;

/// This is a wrapper type introduced to be able to implement traits from `rust-dlc` on the
/// `bdk_ldk::LightningWallet`.
///
/// We want to eventually get rid of the dependency on `bdk-ldk`, because it's a dead project.
pub struct LnDlcWallet(bdk_ldk::LightningWallet<ElectrumBlockchain, sled::Tree>);

impl LnDlcWallet {
    pub fn new(
        blockchain_client: Box<ElectrumBlockchain>,
        wallet: bdk::Wallet<bdk::sled::Tree>,
    ) -> Self {
        Self(bdk_ldk::LightningWallet::new(blockchain_client, wallet))
    }

    // TODO: Better to keep this private and expose the necessary APIs instead.
    pub(crate) fn inner(&self) -> &bdk_ldk::LightningWallet<ElectrumBlockchain, sled::Tree> {
        &self.0
    }

    pub (crate) fn tip(&self) -> anyhow::Result<(u32, BlockHeader)> {
        let (height,header) = self.0.get_tip()?;
        Ok((height, header))
    }
}

impl Signer for LnDlcWallet {
    fn sign_tx_input(
        &self,
        _tx: &mut Transaction,
        _input_index: usize,
        _tx_out: &TxOut,
        _redeem_script: Option<Script>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn get_secret_key_for_pubkey(&self, _pubkey: &PublicKey) -> Result<SecretKey, Error> {
        todo!()
    }
}

impl dlc_manager::Wallet for LnDlcWallet {
    fn get_new_address(&self) -> Result<Address, Error> {
        let address_info = self
            .0
            .get_wallet()
            .unwrap()
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError(Box::new(e)))?;
        Ok(address_info.address)
    }

    fn get_new_secret_key(&self) -> Result<SecretKey, Error> {
        todo!()
    }

    fn get_utxos_for_amount(
        &self,
        _amount: u64,
        _fee_rate: Option<u64>,
        _lock_utxos: bool,
    ) -> Result<Vec<Utxo>, Error> {
        todo!()
    }

    fn import_address(&self, _address: &Address) -> Result<(), Error> {
        Ok(())
    }
}

impl lightning::chain::chaininterface::BroadcasterInterface for LnDlcWallet {
    fn broadcast_transaction(&self, tx: &Transaction) {
        self.0.broadcast_transaction(tx)
    }
}

impl lightning::chain::chaininterface::FeeEstimator for LnDlcWallet {
    fn get_est_sat_per_1000_weight(
        &self,
        confirmation_target: lightning::chain::chaininterface::ConfirmationTarget,
    ) -> u32 {
        self.0.get_est_sat_per_1000_weight(confirmation_target)
    }
}
