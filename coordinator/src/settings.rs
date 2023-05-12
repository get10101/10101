use crate::node::NodeSettings;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

const SETTINGS_FILE_PATH: &str = "coordinator-settings.toml";

/// Top level settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    jit_channels_enabled: bool,
    new_positions_enabled: bool,
    // in sats per vbyte
    max_tx_fee: u64,
    jit_fee_percent: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            jit_channels_enabled: true,
            new_positions_enabled: true,
            max_tx_fee: 20,
            jit_fee_percent: 0.05,
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
}
