use crate::dlc_custom_signer::CustomKeysManager;
use crate::dlc_wallet::DlcWallet;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::node::channel_manager::ChannelManager;
use crate::node::dlc_manager::DlcManager;
use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use crate::ChainMonitor;
use crate::CustomSigner;
use anyhow::Result;
use dlc_manager::sub_channel_manager;
use dlc_manager::SystemTimeProvider;
use ln_dlc_storage::DlcStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::sync::Arc;

pub type SubChannelManager<D, S, N> = sub_channel_manager::SubChannelManager<
    Arc<DlcWallet<D, S, N>>,
    Arc<ChannelManager<D, S, N>>,
    Arc<ChainMonitor<S, N>>,
    Arc<DlcStorageProvider<S>>,
    Arc<DlcWallet<D, S, N>>,
    Arc<P2PDOracleClient>,
    Arc<SystemTimeProvider>,
    Arc<FeeRateEstimator>,
    Arc<DlcManager<D, S, N>>,
    CustomSigner,
    Arc<CustomKeysManager<D>>,
    CustomSigner,
    Arc<CustomKeysManager<D>>,
>;

pub(crate) fn build<D: BdkStorage, S: TenTenOneStorage, N: Storage>(
    channel_manager: Arc<ChannelManager<D, S, N>>,
    dlc_manager: Arc<DlcManager<D, S, N>>,
    monitor: Arc<ChainMonitor<S, N>>,
    provider: Arc<CustomKeysManager<D>>,
) -> Result<Arc<SubChannelManager<D, S, N>>> {
    Ok(Arc::new(SubChannelManager::new(
        channel_manager,
        dlc_manager,
        monitor,
        provider,
    )?))
}
