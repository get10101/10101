use crate::dlc_custom_signer::CustomKeysManager;
use crate::ln::TracingLogger;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::ChainMonitor;
use crate::ConfirmableMonitor;
use anyhow::Result;
use bitcoin::BlockHash;
use lightning::chain::BestBlock;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::channelmanager::ChannelManagerReadArgs;
use lightning::util::config::UserConfig;
use lightning::util::ser::ReadableArgs;
use lightning_persister::FilesystemPersister;
use std::sync::Arc;

pub type ChannelManager = lightning::ln::channelmanager::ChannelManager<
    Arc<ChainMonitor>,
    Arc<LnDlcWallet>,
    Arc<CustomKeysManager>,
    Arc<LnDlcWallet>,
    Arc<TracingLogger>,
>;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn build(
    ldk_data_dir: &str,
    keys_manager: Arc<CustomKeysManager>,
    ln_dlc_wallet: Arc<LnDlcWallet>,
    logger: Arc<TracingLogger>,
    chain_monitor: Arc<ChainMonitor>,
    ldk_user_config: UserConfig,
    network: bitcoin::Network,
    persister: Arc<FilesystemPersister>,
) -> Result<ChannelManager> {
    let file = std::fs::File::open(format!("{ldk_data_dir}/manager")).ok();

    let mut file = match file {
        Some(file) => {
            tracing::info!("Found channel manager data on disk. Recovering from stored state");
            file
        }
        None => {
            tracing::info!(
                "Did not find channel manager data on disk. Initializing new channel manager"
            );

            let (height, block_hash) = ln_dlc_wallet.tip().await?;
            return Ok(ChannelManager::new(
                ln_dlc_wallet.clone(),
                chain_monitor.clone(),
                ln_dlc_wallet,
                logger,
                keys_manager,
                ldk_user_config,
                ChainParameters {
                    network,
                    best_block: BestBlock::new(block_hash, height),
                },
            ));
        }
    };

    let mut channelmonitors = persister.read_channelmonitors(keys_manager.clone())?;

    let mut channel_monitor_mut_references = Vec::new();
    for (_, channel_monitor) in channelmonitors.iter_mut() {
        channel_monitor_mut_references.push(channel_monitor);
    }
    let read_args = ChannelManagerReadArgs::new(
        keys_manager,
        ln_dlc_wallet.clone(),
        chain_monitor.clone(),
        ln_dlc_wallet.clone(),
        logger.clone(),
        ldk_user_config,
        channel_monitor_mut_references,
    );
    let channel_manager = <(BlockHash, ChannelManager)>::read(&mut file, read_args)
        .map_err(|e| anyhow::anyhow!(e))?
        .1;

    // Make sure our filter is initialized with all the txs and outputs
    // that we need to be watching based on our set of channel monitors
    for (_, monitor) in channelmonitors.iter() {
        monitor.load_outputs_to_watch(&ln_dlc_wallet.clone());
    }

    Ok(channel_manager)
}
