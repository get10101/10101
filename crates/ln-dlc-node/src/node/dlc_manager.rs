use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln_dlc_wallet::LnDlcWallet;
use anyhow::Context;
use anyhow::Result;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
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
    Arc<FeeRateEstimator>,
>;

pub fn build(
    data_dir: &Path,
    ln_dlc_wallet: Arc<LnDlcWallet>,
    storage: Arc<SledStorageProvider>,
    p2pdoracle: Arc<P2PDOracleClient>,
    fee_rate_estimator: Arc<FeeRateEstimator>,
) -> Result<DlcManager> {
    let offers_path = data_dir.join("offers");
    fs::create_dir_all(offers_path)?;

    let oracle_pubkey = p2pdoracle.get_public_key();
    let oracles = HashMap::from([(oracle_pubkey, p2pdoracle)]);

    DlcManager::new(
        ln_dlc_wallet.clone(),
        ln_dlc_wallet,
        storage,
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
