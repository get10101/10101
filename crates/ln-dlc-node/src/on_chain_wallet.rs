use crate::seed::WalletSeed;
use anyhow::Context;
use bdk::bitcoin::secp256k1::Secp256k1;
use bdk::sled;
use bdk::wallet::wallet_name_from_descriptor;
use bdk::KeychainKind;
use std::path::Path;

pub struct OnChainWallet {
    pub inner: bdk::Wallet<sled::Tree>,
}

impl OnChainWallet {
    pub fn new(
        data_dir: &Path,
        network: bitcoin::Network,
        seed: WalletSeed,
    ) -> Result<OnChainWallet, anyhow::Error> {
        tracing::info!(?network, "Creating the wallet");

        let data_dir = data_dir.join(&network.to_string());
        dbg!(&data_dir);
        if !data_dir.exists() {
            // TODO: Had to create the `on_chain` directory manually for this to work in the tests
            std::fs::create_dir(&data_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        let ext_priv_key = seed.derive_extended_priv_key(network)?;

        let wallet_name = wallet_name_from_descriptor(
            bdk::template::Bip84(ext_priv_key, KeychainKind::External),
            Some(bdk::template::Bip84(ext_priv_key, KeychainKind::Internal)),
            ext_priv_key.network,
            &Secp256k1::new(),
        )?;

        // Create a database (using default sled type) to store wallet data
        let db = bdk::sled::open(data_dir.join("wallet"))?;
        let db = db.open_tree(wallet_name)?;

        let bdk_wallet = bdk::Wallet::new(
            bdk::template::Bip84(ext_priv_key, KeychainKind::External),
            Some(bdk::template::Bip84(ext_priv_key, KeychainKind::Internal)),
            ext_priv_key.network,
            db,
        )?;

        Ok(OnChainWallet { inner: bdk_wallet })
    }
}
