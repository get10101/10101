use crate::node::NodeSettings;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
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
    pub new_positions_enabled: bool,
    /// Defines the sats/vbyte to be used for all transactions within the sub-channel
    pub contract_tx_fee_rate: u64,

    pub ln_dlc: LnDlcNodeSettings,

    // We don't want the doc block below to be auto-formatted.
    #[rustfmt::skip]
    /// A cron syntax for sending notifications about the rollover window being open
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub rollover_window_open_scheduler: String,

    // We don't want the doc block below to be auto-formatted.
    #[rustfmt::skip]
    /// A cron syntax for sending notifications about the rollover window being open
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub rollover_window_close_scheduler: String,

    // We don't want the doc block below to be auto-formatted.
    #[rustfmt::skip]
    /// A cron syntax for sending notifications to close an expired position
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub close_expired_position_scheduler: String,

    // We don't want the doc block below to be auto-formatted.
    #[rustfmt::skip]
    /// A cron syntax for sending notifications to close an expired position
    ///
    /// The format is :
    /// sec   min   hour   day of month   month   day of week   year
    /// *     *     *      *              *       *             *
    pub close_liquidated_position_scheduler: String,

    // Location of the settings file in the file system.
    path: PathBuf,

    /// If enabled, only makers in [`whitelisted_makers`] are allowed to post limit orders
    pub whitelist_enabled: bool,

    /// A list of makers who are allowed to post limit orders. This is to prevent spam.
    pub whitelisted_makers: Vec<PublicKey>,

    pub min_quantity: u64,

    pub margin_call_percentage: f32,
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
            margin_call_percentage: self.margin_call_percentage,
        }
    }

    pub fn update(&mut self, file: SettingsFile) {
        *self = Self::from_file(file, self.path.clone());
    }

    fn from_file(file: SettingsFile, path: PathBuf) -> Self {
        Self {
            new_positions_enabled: file.new_positions_enabled,
            contract_tx_fee_rate: file.contract_tx_fee_rate,
            ln_dlc: file.ln_dlc,
            rollover_window_open_scheduler: file.rollover_window_open_scheduler,
            rollover_window_close_scheduler: file.rollover_window_close_scheduler,
            close_expired_position_scheduler: file.close_expired_position_scheduler,
            close_liquidated_position_scheduler: file.close_liquidated_position_scheduler,
            path,
            whitelist_enabled: file.whitelist_enabled,
            whitelisted_makers: file.whitelisted_makers,
            min_quantity: file.min_quantity,
            margin_call_percentage: file.margin_call_percentage,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SettingsFile {
    new_positions_enabled: bool,

    contract_tx_fee_rate: u64,

    ln_dlc: LnDlcNodeSettings,

    rollover_window_open_scheduler: String,
    rollover_window_close_scheduler: String,

    close_expired_position_scheduler: String,
    close_liquidated_position_scheduler: String,

    whitelist_enabled: bool,
    whitelisted_makers: Vec<PublicKey>,

    min_quantity: u64,
    margin_call_percentage: f32,
}

impl From<Settings> for SettingsFile {
    fn from(value: Settings) -> Self {
        Self {
            new_positions_enabled: value.new_positions_enabled,
            contract_tx_fee_rate: value.contract_tx_fee_rate,
            ln_dlc: value.ln_dlc,
            rollover_window_open_scheduler: value.rollover_window_open_scheduler,
            rollover_window_close_scheduler: value.rollover_window_close_scheduler,
            close_expired_position_scheduler: value.close_expired_position_scheduler,
            close_liquidated_position_scheduler: value.close_liquidated_position_scheduler,
            whitelist_enabled: false,
            whitelisted_makers: value.whitelisted_makers,
            min_quantity: value.min_quantity,
            margin_call_percentage: value.margin_call_percentage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn toml_serde_roundtrip() {
        let original = SettingsFile {
            new_positions_enabled: true,
            contract_tx_fee_rate: 1,
            ln_dlc: LnDlcNodeSettings {
                off_chain_sync_interval: std::time::Duration::from_secs(1),
                on_chain_sync_interval: std::time::Duration::from_secs(1),
                fee_rate_sync_interval: std::time::Duration::from_secs(1),
                sub_channel_manager_periodic_check_interval: std::time::Duration::from_secs(1),
                shadow_sync_interval: std::time::Duration::from_secs(1),
            },
            rollover_window_open_scheduler: "foo".to_string(),
            rollover_window_close_scheduler: "bar".to_string(),
            close_expired_position_scheduler: "baz".to_string(),
            close_liquidated_position_scheduler: "baz".to_string(),
            whitelist_enabled: false,
            whitelisted_makers: vec![PublicKey::from_str(
                "0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166",
            )
            .unwrap()],
            min_quantity: 1,
            margin_call_percentage: 0.1,
        };

        let serialized = toml::to_string_pretty(&original).unwrap();

        let deserialized = toml::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }
}
