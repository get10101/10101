use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::channel_manager::ChannelManager;
use crate::node::dlc_manager::DlcManager;
use anyhow::Result;
use bitcoin::secp256k1::Secp256k1;
use dlc_manager::SystemTimeProvider;
use dlc_sled_storage_provider::SledStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::sync::Arc;

pub(crate) type SubChannelManager = dlc_manager::sub_channel_manager::SubChannelManager<
    Arc<LnDlcWallet>,
    Arc<ChannelManager>,
    Arc<SledStorageProvider>,
    Arc<LnDlcWallet>,
    Arc<P2PDOracleClient>,
    Arc<SystemTimeProvider>,
    Arc<LnDlcWallet>,
    Arc<DlcManager>,
>;

pub(crate) fn build(
    dlc_manager: Arc<DlcManager>,
    ln_dlc_wallet: Arc<LnDlcWallet>,
    channel_manager: Arc<ChannelManager>,
    storage: Arc<SledStorageProvider>,
) -> Result<Arc<SubChannelManager>> {
    let (height, _) = ln_dlc_wallet.tip()?;
    Ok(Arc::new(SubChannelManager::new(
        Secp256k1::new(),
        ln_dlc_wallet.clone(),
        channel_manager.clone(),
        storage,
        ln_dlc_wallet.clone(),
        dlc_manager,
        ln_dlc_wallet,
        height as u64,
    )))
}
