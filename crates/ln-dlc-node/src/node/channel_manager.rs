use crate::dlc_custom_signer::CustomKeysManager;
use crate::ln::TracingLogger;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::ChainMonitor;
use crate::ConfirmableMonitor;
use anyhow::Result;
use bitcoin::BlockHash;
use lightning::chain::BestBlock;
use lightning::chain::ChannelMonitorUpdateStatus;
use lightning::chain::Confirm;
use lightning::chain::Watch;
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
pub(crate) fn build(
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

            let (height, header) = ln_dlc_wallet.tip()?;
            return Ok(ChannelManager::new(
                ln_dlc_wallet.clone(),
                chain_monitor.clone(),
                ln_dlc_wallet,
                logger,
                keys_manager,
                ldk_user_config,
                ChainParameters {
                    network,
                    best_block: BestBlock::new(header.block_hash(), height),
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

    // `Confirm` trait is not implemented on an individual ChannelMonitor
    // but on a tuple consisting of (channel_monitor, broadcaster, fee_estimator, logger)
    // this maps our channel monitors into a tuple that implements Confirm
    let mut confirmable_monitors = channelmonitors
        .into_iter()
        .map(|(_, channel_monitor)| {
            (
                channel_monitor,
                ln_dlc_wallet.clone(),
                ln_dlc_wallet.clone(),
                logger.clone(),
            )
        })
        .collect::<Vec<ConfirmableMonitor>>();

    // construct and collect a Vec of references to objects that implement the Confirm trait
    // note: we chain the channel_manager into this Vec
    let confirmables: Vec<&dyn Confirm> = confirmable_monitors
        .iter()
        .map(|cm| cm as &dyn Confirm)
        .chain(std::iter::once(&channel_manager as &dyn Confirm))
        .collect();

    // Sync our channel monitors and channel manager to chain tip
    ln_dlc_wallet.inner().sync(confirmables)?;

    // Give ChannelMonitors to ChainMonitor to watch
    for confirmable_monitor in confirmable_monitors.drain(..) {
        let channel_monitor = confirmable_monitor.0;

        // ATTENTION: This must be `get_original_funding_txo` and _not_ `get_funding_txo`, because
        // we are using LN-DLC channels. `rust-dlc` is manipulating the funding TXO so that LDK
        // considers the `glue_transaction` as the `funding_transaction` for certain purposes.
        //
        // For other purposes, LDK must still refer back to the original `funding_transaction`. This
        // is one such case.
        let funding_txo = channel_monitor.get_original_funding_txo().0;
        assert_eq!(
            chain_monitor.watch_channel(funding_txo, channel_monitor),
            ChannelMonitorUpdateStatus::Completed
        );
    }

    Ok(channel_manager)
}
