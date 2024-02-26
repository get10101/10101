use crate::bitcoin_conversion::to_secp_pk_29;
use crate::dlc_wallet::DlcWallet;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::node::Node;
use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::Storage as DlcStorage;
use dlc_manager::SystemTimeProvider;
use ln_dlc_storage::DlcStorageProvider;
use p2pd_oracle_client::P2PDOracleClient;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub type DlcManager<D, S, N> = dlc_manager::manager::Manager<
    Arc<DlcWallet<D, S, N>>,
    Arc<DlcWallet<D, S, N>>,
    Arc<DlcStorageProvider<S>>,
    Arc<P2PDOracleClient>,
    Arc<SystemTimeProvider>,
    Arc<FeeRateEstimator>,
>;

pub fn build<D: BdkStorage, S: TenTenOneStorage, N: Storage>(
    data_dir: &Path,
    wallet: Arc<DlcWallet<D, S, N>>,
    dlc_storage: Arc<DlcStorageProvider<S>>,
    p2pdoracles: Vec<Arc<P2PDOracleClient>>,
    fee_rate_estimator: Arc<FeeRateEstimator>,
) -> Result<DlcManager<D, S, N>> {
    let offers_path = data_dir.join("offers");
    fs::create_dir_all(offers_path)?;

    let mut oracles = HashMap::new();
    for oracle in p2pdoracles.into_iter() {
        oracles.insert(oracle.public_key, oracle);
    }

    // FIXME: We need to do this to ensure that we can upgrade `Node`s from LDK 0.0.114 to 0.0.116.
    // We should remove this workaround as soon as possible.
    if let Err(e) = dlc_storage.get_chain_monitor() {
        tracing::error!("Failed to load DLC ChainMonitor from storage: {e:#}");

        tracing::info!("Overwriting old DLC ChainMonitor with empty one to be able to proceed");
        dlc_storage.persist_chain_monitor(&dlc_manager::chain_monitor::ChainMonitor::new(0))?;
    }

    DlcManager::new(
        wallet.clone(),
        wallet,
        dlc_storage,
        oracles,
        Arc::new(SystemTimeProvider {}),
        fee_rate_estimator,
    )
    .context("Failed to initialise DlcManager")
}

pub fn signed_channel_state_name(signed_channel: &SignedChannel) -> String {
    let name = match signed_channel.state {
        SignedChannelState::Established { .. } => "Established",
        SignedChannelState::SettledOffered { .. } => "SettledOffered",
        SignedChannelState::SettledReceived { .. } => "SettledReceived",
        SignedChannelState::SettledAccepted { .. } => "SettledAccepted",
        SignedChannelState::SettledConfirmed { .. } => "SettledConfirmed",
        SignedChannelState::Settled { .. } => "Settled",
        SignedChannelState::RenewOffered { .. } => "RenewOffered",
        SignedChannelState::RenewAccepted { .. } => "RenewAccepted",
        SignedChannelState::RenewConfirmed { .. } => "RenewConfirmed",
        SignedChannelState::RenewFinalized { .. } => "RenewFinalized",
        SignedChannelState::Closing { .. } => "Closing",
        SignedChannelState::CollaborativeCloseOffered { .. } => "CollaborativeCloseOffered",
    };

    name.to_string()
}

impl<D: BdkStorage, S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static>
    Node<D, S, N>
{
    pub fn get_signed_channel_by_trader_id(&self, trader_id: PublicKey) -> Result<SignedChannel> {
        let dlc_channels = self.list_signed_dlc_channels()?;
        let signed_channel = dlc_channels
            .iter()
            .find(|channel| channel.counter_party == to_secp_pk_29(trader_id))
            .context(format!(
                "Could not find a signed dlc channel for trader {trader_id}",
            ))?;

        Ok(signed_channel.clone())
    }
}
