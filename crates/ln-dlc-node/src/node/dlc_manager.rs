use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::Node;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::Oracle;
use dlc_manager::Storage;
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

    // FIXME: We need to do this to ensure that we can upgrade `Node`s from LDK 0.0.114 to 0.0.116.
    // We should remove this workaround as soon as possible.
    if let Err(e) = storage.get_chain_monitor() {
        tracing::error!("Failed to load DLC ChainMonitor from storage: {e:#}");

        tracing::info!("Overwriting old DLC ChainMonitor with empty one to be able to proceed");
        storage.persist_chain_monitor(&dlc_manager::chain_monitor::ChainMonitor::new(0))?;
    }

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

impl<P> Node<P>
where
    P: Send + Sync,
{
    pub fn get_signed_channel_by_trader_id(&self, trader_id: PublicKey) -> Result<SignedChannel> {
        let channel = self
            .get_dlc_channel_signed(&trader_id)?
            .with_context(|| format!("Could not find signed DLC channel. trader_id={trader_id}"))?;

        let dlc_channel_id = channel
            .get_dlc_channel_id(0)
            .expect("Expect to get dlc_channel id");

        let channel = self.get_dlc_channel_by_id(&dlc_channel_id)?;
        let signed_channel = match channel {
            Channel::Signed(signed_channel) => signed_channel,
            _ => bail!("Couldn't find signed channel for trader_id={trader_id}"),
        };

        Ok(signed_channel)
    }
}
