use bdk::blockchain::ElectrumBlockchain;
use bdk::sled;
use bdk::wallet::AddressIndex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Address;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::TxOut;
use dlc_manager::error::Error;
use dlc_manager::error::Error::WalletError;
use dlc_manager::Signer;
use dlc_manager::Utxo;
use dlc_manager::Wallet;
use std::ops::Deref;
use std::sync::Arc;

pub struct BDKLDKWallet {
    pub inner: Arc<bdk_ldk::LightningWallet<ElectrumBlockchain, sled::Tree>>,
}

impl Deref for BDKLDKWallet {
    type Target = ();

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl Signer for BDKLDKWallet {
    fn sign_tx_input(
        &self,
        tx: &mut Transaction,
        input_index: usize,
        tx_out: &TxOut,
        redeem_script: Option<Script>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn get_secret_key_for_pubkey(&self, pubkey: &PublicKey) -> Result<SecretKey, Error> {
        todo!()
    }
}

impl Wallet for BDKLDKWallet {
    fn get_new_address(&self) -> Result<Address, Error> {
        let address_info = self
            .inner
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
        amount: u64,
        fee_rate: Option<u64>,
        lock_utxos: bool,
    ) -> Result<Vec<Utxo>, Error> {
        todo!()
    }

    fn import_address(&self, address: &Address) -> Result<(), Error> {
        Ok(())
    }
}
