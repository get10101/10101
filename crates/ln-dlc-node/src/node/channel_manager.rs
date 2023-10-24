use crate::dlc_custom_signer::CustomKeysManager;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln::TracingLogger;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::ChainMonitor;
use crate::Router;
use anyhow::Result;
use bitcoin::BlockHash;
use lightning::chain::BestBlock;
use lightning::chain::ChannelMonitorUpdateStatus;
use lightning::chain::Watch;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::channelmanager::ChannelManagerReadArgs;
use lightning::util::config::UserConfig;
use lightning::util::ser::ReadableArgs;
use lightning_persister::FilesystemPersister;
use lightning_transaction_sync::EsploraSyncClient;
use std::sync::Arc;

pub type ChannelManager = lightning::ln::channelmanager::ChannelManager<
    Arc<ChainMonitor>,
    Arc<LnDlcWallet>,
    Arc<CustomKeysManager>,
    Arc<CustomKeysManager>,
    Arc<CustomKeysManager>,
    Arc<FeeRateEstimator>,
    Arc<Router>,
    Arc<TracingLogger>,
>;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    ldk_data_dir: &str,
    keys_manager: Arc<CustomKeysManager>,
    ln_dlc_wallet: Arc<LnDlcWallet>,
    fee_rate_estimator: Arc<FeeRateEstimator>,
    explora_client: Arc<EsploraSyncClient<Arc<TracingLogger>>>,
    logger: Arc<TracingLogger>,
    chain_monitor: Arc<ChainMonitor>,
    ldk_config: UserConfig,
    network: bitcoin::Network,
    persister: Arc<FilesystemPersister>,
    router: Arc<Router>,
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

            let (height, block_hash) = ln_dlc_wallet.tip()?;
            return Ok(ChannelManager::new(
                fee_rate_estimator,
                chain_monitor.clone(),
                ln_dlc_wallet,
                router,
                logger,
                keys_manager.clone(),
                keys_manager.clone(),
                keys_manager,
                ldk_config,
                ChainParameters {
                    network,
                    best_block: BestBlock::new(block_hash, height),
                },
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as u32,
            ));
        }
    };

    let mut channelmonitors =
        persister.read_channelmonitors(keys_manager.clone(), keys_manager.clone())?;

    let mut channel_monitor_mut_references = Vec::new();
    for (_, channel_monitor) in channelmonitors.iter_mut() {
        channel_monitor_mut_references.push(channel_monitor);
    }
    let read_args = ChannelManagerReadArgs::new(
        keys_manager.clone(),
        keys_manager.clone(),
        keys_manager,
        fee_rate_estimator,
        chain_monitor.clone(),
        ln_dlc_wallet,
        router,
        logger,
        ldk_config,
        channel_monitor_mut_references,
    );
    let channel_manager = <(BlockHash, ChannelManager)>::read(&mut file, read_args)
        .map_err(|e| anyhow::anyhow!(e))?
        .1;

    // Make sure our filter is initialized with all the txs and outputs
    // that we need to be watching based on our set of channel monitors
    for (_, monitor) in channelmonitors.iter() {
        monitor.load_outputs_to_watch(&explora_client.clone());
    }

    for (_, monitor) in channelmonitors.drain(..) {
        // ATTENTION: This must be `get_original_funding_txo` and _not_ `get_funding_txo`, because
        // we are using LN-DLC channels. `rust-dlc` is manipulating the funding TXO so that LDK
        // considers the `glue_transaction` as the `funding_transaction` for certain purposes.
        //
        // For other purposes, LDK must still refer back to the original `funding_transaction`. This
        // is one such case.
        let funding_txo = monitor.get_original_funding_txo().0;
        assert_eq!(
            chain_monitor.watch_channel(funding_txo, monitor),
            ChannelMonitorUpdateStatus::Completed
        );
    }

    Ok(channel_manager)
}
