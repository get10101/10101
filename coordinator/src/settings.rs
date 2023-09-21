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
#[derive(Debug, Clone, Deserialize, Serialize)]
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

    /// Amount (in millionths of a satoshi) charged per satoshi for payments forwarded outbound
    /// over a channel.
    pub forwarding_fee_proportional_millionths: u32,

    // Special parameter, where the settings file is located
    pub path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            jit_channels_enabled: true,
            new_positions_enabled: true,
            contract_tx_fee_rate: 9,
            fallback_tx_fee_rate_normal: 2000,
            fallback_tx_fee_rate_high_priority: 5000,
            max_allowed_tx_fee_rate_when_opening_channel: None,
            ln_dlc: LnDlcNodeSettings::default(),
            forwarding_fee_proportional_millionths: 50,
            path: None,
        }
    }
}

async fn read_settings(data_dir: &Path) -> Result<Settings> {
    let settings_path = data_dir.join(SETTINGS_FILE_NAME);
    let data = fs::read_to_string(settings_path).await?;
    toml::from_str(&data).context("Unable to parse settings file")
}

impl Settings {
    pub async fn new(data_dir: &Path) -> Self {
        match read_settings(data_dir).await {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("Unable to read {SETTINGS_FILE_NAME} file, using defaults: {e}");
                let new = Settings {
                    path: Some(data_dir.join(SETTINGS_FILE_NAME)),
                    ..Settings::default()
                };
                if let Err(e) = new.write_to_file().await {
                    tracing::error!("Unable to write default settings to file: {e}");
                } else {
                    tracing::info!("Default settings written to file");
                }
                new
            }
        }
    }

    pub async fn write_to_file(&self) -> Result<()> {
        let data =
            toml::to_string_pretty(&self).context("Unable to serialize settings to TOML format")?;

        let settings_path = self.path.as_ref().context("Settings path not set")?.clone();
        let mut file = fs::File::create(settings_path).await?;
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
        }
    }

    /// The part of the coordinator settings pertaining to the LDK node.
    pub fn to_ldk_settings(&self) -> UserConfig {
        // Since we currently have to keep the coordinator settings in sync with the tests in
        // `ln-dlc-node`, we let the library define the default settings (which is bad)
        let mut ldk_config = ln_dlc_node::config::coordinator_config();

        ldk_config
            .channel_config
            .forwarding_fee_proportional_millionths = self.forwarding_fee_proportional_millionths;

        ldk_config
    }
}
