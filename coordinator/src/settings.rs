use crate::node::NodeSettings;
use anyhow::Context;
use anyhow::Result;
use lightning::util::config::UserConfig;
use ln_dlc_node::node::LnDlcNodeSettings;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

const SETTINGS_FILE_NAME: &str = "coordinator-settings.toml";

/// Top-level settings.
#[derive(Debug, Clone, Serialize)]
pub struct Settings {
    pub jit_channels_enabled: bool,
    pub new_positions_enabled: bool,
    /// Defines the sats/vbyte to be used for all transactions within the sub-channel
    pub contract_tx_fee_rate: u64,
    pub fallback_tx_fee_rate_normal: u32,
    pub fallback_tx_fee_rate_high_priority: u32,

    /// At times, we might want to prevent opening new channels if the fee rate is too high.
    /// If set to `None`, no limit is enforced.
    //  In sats/kWU (weight unit)
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,

    pub ln_dlc: LnDlcNodeSettings,

    /// We don't want the below doc block be formatted
    #[rustfmt::skip]
    /// A cron syntax for sending notifications about the rollover window being open
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub rollover_window_open_scheduler: String,

    /// We don't want the below doc block be formatted
    #[rustfmt::skip]
    /// A cron syntax for sending notifications about the rollover window being open
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub rollover_window_close_scheduler: String,

    /// We don't want the below doc block be formatted
    #[rustfmt::skip]
    /// A cron syntax for sending notifications to close an expired position
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub close_expired_position_scheduler: String,

    /// Min balance to keep in on-chain wallet at all times
    pub min_liquidity_threshold_sats: u64,

    // Location of the settings file in the file system.
    path: PathBuf,
}

impl Settings {
    pub async fn new(data_dir: &Path) -> Result<Self> {
        let settings_path = data_dir.join(SETTINGS_FILE_NAME);

        let data = fs::read_to_string(&settings_path)
            .await
            .with_context(|| format!("Failed to read settings at {settings_path:?}"))?;

        let settings =
            toml::from_str::<SettingsFile>(&data).context("Unable to parse settings file")?;
        let settings = Self::from_file(settings, settings_path);

        tracing::info!(?settings, "Read settings from file system");

        Ok(settings)
    }

    pub async fn write_to_file(&self) -> Result<()> {
        let data = toml::to_string_pretty(&SettingsFile::from(self.clone()))
            .context("Unable to serialize settings to TOML format")?;

        let mut file = fs::File::create(&self.path).await?;
        file.write_all(data.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    /// Return the node settings part of the settings file
    pub fn to_node_settings(&self) -> NodeSettings {
        NodeSettings {
            allow_opening_positions: self.new_positions_enabled,
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
            contract_tx_fee_rate: self.contract_tx_fee_rate,
            jit_channels_enabled: self.jit_channels_enabled,
        }
    }

    /// The part of the coordinator settings pertaining to the LDK node.
    pub fn to_ldk_settings(&self) -> UserConfig {
        // Since we currently have to keep the coordinator settings in sync with the tests in
        // `ln-dlc-node`, we let the library define the default settings (which is bad)
        let mut ldk_config = ln_dlc_node::config::coordinator_config();

        ldk_config
            .channel_config
            .forwarding_fee_proportional_millionths =
            self.ln_dlc.forwarding_fee_proportional_millionths;

        ldk_config
    }

    pub fn update(&mut self, file: SettingsFile) {
        *self = Self::from_file(file, self.path.clone());
    }

    fn from_file(file: SettingsFile, path: PathBuf) -> Self {
        Self {
            jit_channels_enabled: file.jit_channels_enabled,
            new_positions_enabled: file.new_positions_enabled,
            contract_tx_fee_rate: file.contract_tx_fee_rate,
            fallback_tx_fee_rate_normal: file.fallback_tx_fee_rate_normal,
            fallback_tx_fee_rate_high_priority: file.fallback_tx_fee_rate_high_priority,
            max_allowed_tx_fee_rate_when_opening_channel: file
                .max_allowed_tx_fee_rate_when_opening_channel,
            ln_dlc: file.ln_dlc,
            rollover_window_open_scheduler: file.rollover_window_open_scheduler,
            rollover_window_close_scheduler: file.rollover_window_close_scheduler,
            close_expired_position_scheduler: file.close_expired_position_scheduler,
            min_liquidity_threshold_sats: file.min_liquidity_threshold_sats,
            path,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsFile {
    jit_channels_enabled: bool,
    new_positions_enabled: bool,

    contract_tx_fee_rate: u64,
    fallback_tx_fee_rate_normal: u32,
    fallback_tx_fee_rate_high_priority: u32,

    max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,

    ln_dlc: LnDlcNodeSettings,

    rollover_window_open_scheduler: String,
    rollover_window_close_scheduler: String,

    close_expired_position_scheduler: String,

    min_liquidity_threshold_sats: u64,
}

impl From<Settings> for SettingsFile {
    fn from(value: Settings) -> Self {
        Self {
            jit_channels_enabled: value.jit_channels_enabled,
            new_positions_enabled: value.new_positions_enabled,
            contract_tx_fee_rate: value.contract_tx_fee_rate,
            fallback_tx_fee_rate_normal: value.fallback_tx_fee_rate_normal,
            fallback_tx_fee_rate_high_priority: value.fallback_tx_fee_rate_high_priority,
            max_allowed_tx_fee_rate_when_opening_channel: value
                .max_allowed_tx_fee_rate_when_opening_channel,
            ln_dlc: value.ln_dlc,
            rollover_window_open_scheduler: value.rollover_window_open_scheduler,
            rollover_window_close_scheduler: value.rollover_window_close_scheduler,
            close_expired_position_scheduler: value.close_expired_position_scheduler,
            min_liquidity_threshold_sats: value.min_liquidity_threshold_sats,
        }
    }
}
