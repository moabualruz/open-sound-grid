use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::ui::theme::ThemeMode;

/// Persisted application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub channels: Vec<ChannelConfig>,
    pub mixes: Vec<MixConfig>,
    pub audio: AudioConfig,
    pub ui: UiConfig,
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
}

/// A persisted route between a channel and a mix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteConfig {
    pub channel_name: String,
    pub mix_name: String,
    pub volume: f32,
    pub enabled: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
    #[serde(default)]
    pub effects: crate::effects::EffectsParams,
    #[serde(default)]
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixConfig {
    pub name: String,
    pub icon: String,
    pub color: [u8; 3],
    pub output_device: Option<String>,
    #[serde(default = "default_volume")]
    pub master_volume: f32,
    #[serde(default)]
    pub muted: bool,
}

fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub latency_ms: u32,
    pub output_device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub compact_mode: bool,
    pub theme_mode: ThemeMode,
    pub window_width: u32,
    pub window_height: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        tracing::debug!("creating default AppConfig");
        Self {
            channels: vec![
                ChannelConfig { name: "Music".into(), effects: Default::default(), muted: false },
                ChannelConfig { name: "Game".into(), effects: Default::default(), muted: false },
                ChannelConfig { name: "Voice".into(), effects: Default::default(), muted: false },
                ChannelConfig { name: "System".into(), effects: Default::default(), muted: false },
            ],
            mixes: vec![
                MixConfig {
                    name: "Monitor".into(),
                    icon: "🎧".into(),
                    color: [100, 149, 237],
                    output_device: None,
                    master_volume: 1.0,
                    muted: false,
                },
                MixConfig {
                    name: "Stream".into(),
                    icon: "📡".into(),
                    color: [255, 99, 71],
                    output_device: None,
                    master_volume: 1.0,
                    muted: false,
                },
            ],
            audio: AudioConfig {
                latency_ms: 20,
                output_device: "auto".into(),
            },
            ui: UiConfig {
                compact_mode: false,
                theme_mode: ThemeMode::Dark,
                window_width: 1000,
                window_height: 600,
            },
            routes: Vec::new(),
        }
    }
}

impl AppConfig {
    #[instrument]
    pub fn load() -> Self {
        match confy::load::<Self>("open-sound-grid", None) {
            Ok(config) => {
                let path = confy::get_configuration_file_path("open-sound-grid", None)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "<unknown>".into());
                tracing::info!(
                    path = %path,
                    channels = config.channels.len(),
                    mixes = config.mixes.len(),
                    routes = config.routes.len(),
                    "config loaded"
                );
                for ch in &config.channels {
                    tracing::debug!(name = %ch.name, "loaded channel config");
                }
                for mix in &config.mixes {
                    tracing::debug!(
                        name = %mix.name,
                        icon = %mix.icon,
                        output_device = ?mix.output_device,
                        "loaded mix config"
                    );
                }
                config
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load config, using defaults");
                Self::default()
            }
        }
    }

    #[instrument(skip(self))]
    pub fn save(&self) -> anyhow::Result<()> {
        confy::store("open-sound-grid", None, self)?;
        let path = confy::get_configuration_file_path("open-sound-grid", None)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        tracing::info!(path = %path, "config saved");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_expected_channels_and_mixes() {
        let config = AppConfig::default();
        assert_eq!(config.channels.len(), 4, "default config must have 4 channels");
        assert_eq!(config.mixes.len(), 2, "default config must have 2 mixes");
        assert_eq!(config.audio.latency_ms, 20, "default latency must be 20 ms");
    }
}
