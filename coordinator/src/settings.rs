use crate::node::NodeSettings;
use anyhow::Context;
use anyhow::Result;
use ln_dlc_node::node::LnDlcNodeSettings;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

const SETTINGS_FILE_NAME: &str = "coordinator-settings.toml";

/// Top level settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    pub jit_channels_enabled: bool,
    pub new_positions_enabled: bool,
    /// Fee rate to be charged for opening just in time channels. Rate is in basis points, i.e.
    /// 100 basis point=1% or 50=0.5%
    pub jit_fee_rate_basis_points: u32,

    pub fallback_tx_fee_rate_normal: u32,
    pub fallback_tx_fee_rate_high_priority: u32,

    /// At times, we might want to prevent opening new channels if the fee rate is too high.
    /// If set to `None`, no limit is enforced.
    //  In sats/kWU (weight unit)
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,

    pub ln_dlc: LnDlcNodeSettings,
    // Special parameter, where the settings file is located
    pub path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            jit_channels_enabled: true,
            new_positions_enabled: true,
            jit_fee_rate_basis_points: 50,
            fallback_tx_fee_rate_normal: 2000,
            fallback_tx_fee_rate_high_priority: 5000,
            max_allowed_tx_fee_rate_when_opening_channel: None,
            ln_dlc: LnDlcNodeSettings::default(),
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
    pub fn as_node_settings(&self) -> NodeSettings {
        NodeSettings {
            allow_opening_positions: self.new_positions_enabled,
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
        }
    }
}
