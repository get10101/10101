use crate::ln_dlc_wallet::LnDlcWallet;
use anyhow::Result;
use dlc_manager::Oracle;
use dlc_manager::SystemTimeProvider;
use dlc_sled_storage_provider::SledStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub type DlcManager = dlc_manager::manager::Manager<
    Arc<LnDlcWallet>,
    Arc<LnDlcWallet>,
    Arc<SledStorageProvider>,
    Arc<P2PDOracleClient>,
    Arc<SystemTimeProvider>,
    Arc<LnDlcWallet>,
>;

pub fn build(
    data_dir: &Path,
    ln_dlc_wallet: Arc<LnDlcWallet>,
    storage: Arc<SledStorageProvider>,
    p2pdoracle: Arc<P2PDOracleClient>,
) -> Result<DlcManager> {
    let offers_path = data_dir.join("offers");
    fs::create_dir_all(offers_path)?;

    let oracle_pubkey = p2pdoracle.get_public_key();
    let oracles = HashMap::from([(oracle_pubkey, p2pdoracle)]);

    DlcManager::new(
        ln_dlc_wallet.clone(),
        ln_dlc_wallet.clone(),
        storage,
        oracles,
        Arc::new(SystemTimeProvider {}),
        ln_dlc_wallet,
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))
}
