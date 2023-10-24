use crate::dlc_custom_signer::CustomKeysManager;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::channel_manager::ChannelManager;
use crate::node::dlc_manager::DlcManager;
use crate::ChainMonitor;
use crate::CustomSigner;
use anyhow::Result;
use dlc_manager::sub_channel_manager;
use dlc_manager::SystemTimeProvider;
use dlc_sled_storage_provider::SledStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::sync::Arc;

pub type SubChannelManager = sub_channel_manager::SubChannelManager<
    Arc<LnDlcWallet>,
    Arc<ChannelManager>,
    Arc<ChainMonitor>,
    Arc<SledStorageProvider>,
    Arc<LnDlcWallet>,
    Arc<P2PDOracleClient>,
    Arc<SystemTimeProvider>,
    Arc<FeeRateEstimator>,
    Arc<DlcManager>,
    CustomSigner,
    Arc<CustomKeysManager>,
    CustomSigner,
>;

pub(crate) fn build(
    channel_manager: Arc<ChannelManager>,
    dlc_manager: Arc<DlcManager>,
    monitor: Arc<ChainMonitor>,
    provider: Arc<CustomKeysManager>,
) -> Result<Arc<SubChannelManager>> {
    Ok(Arc::new(SubChannelManager::new(
        channel_manager,
        dlc_manager,
        monitor,
        provider,
    )?))
}
