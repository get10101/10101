use crate::dlc_custom_signer::CustomKeysManager;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::channel_manager::ChannelManager;
use crate::node::dlc_manager::DlcManager;
use crate::node::Storage;
use crate::storage::TenTenOneStorage;
use crate::ChainMonitor;
use crate::CustomSigner;
use anyhow::Result;
use dlc_manager::sub_channel_manager;
use dlc_manager::SystemTimeProvider;
use ln_dlc_storage::DlcStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::sync::Arc;

pub type SubChannelManager<S, N> = sub_channel_manager::SubChannelManager<
    Arc<LnDlcWallet<S, N>>,
    Arc<ChannelManager<S, N>>,
    Arc<ChainMonitor<S, N>>,
    Arc<DlcStorageProvider<S>>,
    Arc<LnDlcWallet<S, N>>,
    Arc<P2PDOracleClient>,
    Arc<SystemTimeProvider>,
    Arc<FeeRateEstimator>,
    Arc<DlcManager<S, N>>,
    CustomSigner,
    Arc<CustomKeysManager<S, N>>,
    CustomSigner,
    Arc<CustomKeysManager<S, N>>,
>;

pub(crate) fn build<S: TenTenOneStorage, N: Storage>(
    channel_manager: Arc<ChannelManager<S, N>>,
    dlc_manager: Arc<DlcManager<S, N>>,
    monitor: Arc<ChainMonitor<S, N>>,
    provider: Arc<CustomKeysManager<S, N>>,
) -> Result<Arc<SubChannelManager<S, N>>> {
    Ok(Arc::new(SubChannelManager::new(
        channel_manager,
        dlc_manager,
        monitor,
        provider,
    )?))
}
