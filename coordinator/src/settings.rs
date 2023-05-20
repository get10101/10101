use crate::node::NodeSettings;
use anyhow::Context;
use anyhow::Result;
use ln_dlc_node::node::LnDlcNodeSettings;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

const SETTINGS_FILE_PATH: &str = "coordinator-settings.toml";

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
        }
    }
}

async fn read_settings() -> Result<Settings> {
    let settings_path = Path::new(SETTINGS_FILE_PATH);
    let data = fs::read_to_string(settings_path).await?;
    toml::from_str(&data).context("Unable to parse settings file")
}

impl Settings {
    pub async fn new() -> Self {
        match read_settings().await {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("Unable to read {SETTINGS_FILE_PATH} file, using defaults: {e}");
                Settings::default()
            }
        }
    }

    pub async fn write_to_file(&self) -> Result<()> {
        let data =
            toml::to_string_pretty(&self).context("Unable to serialize settings to TOML format")?;

        let settings_path = Path::new(SETTINGS_FILE_PATH);
        let mut file = fs::File::create(settings_path).await?;
        file.write_all(data.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    /// Return the node settings part of the settings file
    pub fn as_node_settings(&self) -> NodeSettings {
        NodeSettings {
            allow_opening_positions: self.new_positions_enabled,
            fallback_tx_fee_rate_normal: self.fallback_tx_fee_rate_normal,
            fallback_tx_fee_rate_high_priority: self.fallback_tx_fee_rate_high_priority,
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
        }
    }
}
