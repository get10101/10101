use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::channel_manager::ChannelManager;
use crate::node::dlc_manager::DlcManager;
use anyhow::Result;
use dlc_manager::sub_channel_manager;
use dlc_manager::SystemTimeProvider;
use dlc_sled_storage_provider::SledStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::sync::Arc;

pub type SubChannelManager = sub_channel_manager::SubChannelManager<
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
    channel_manager: Arc<ChannelManager>,
    dlc_manager: Arc<DlcManager>,
    height: u64,
) -> Result<Arc<SubChannelManager>> {
    Ok(Arc::new(SubChannelManager::new(
        channel_manager.clone(),
        dlc_manager,
        height,
    )))
}
