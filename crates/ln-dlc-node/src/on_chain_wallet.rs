use crate::seed::WalletSeed;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::Secp256k1;
use bdk::wallet::wallet_name_from_descriptor;
use bdk::KeychainKind;
use std::path::Path;

pub fn new_bdk_wallet(
    data_dir: &Path,
    network: bitcoin::Network,
    seed: WalletSeed,
) -> Result<bdk::Wallet<bdk::sled::Tree>> {
    tracing::info!(?network, "Creating the wallet");

    let data_dir = data_dir.join(network.to_string());
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).context(format!(
            "Could not create data dir ({data_dir:?}) for {network}"
        ))?;
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

    Ok(bdk_wallet)
}
