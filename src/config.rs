use serde::{Deserialize, Serialize};

/// Persisted application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub channels: Vec<ChannelConfig>,
    pub mixes: Vec<MixConfig>,
    pub audio: AudioConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixConfig {
    pub name: String,
    pub icon: String,
    pub color: [u8; 3],
    pub output_device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub latency_ms: u32,
    pub output_device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub compact_mode: bool,
    pub window_width: u32,
    pub window_height: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        tracing::debug!("creating default AppConfig");
        Self {
            channels: vec![
                ChannelConfig { name: "Music".into() },
                ChannelConfig { name: "Game".into() },
                ChannelConfig { name: "Voice".into() },
                ChannelConfig { name: "System".into() },
            ],
            mixes: vec![
                MixConfig {
                    name: "Monitor".into(),
                    icon: "🎧".into(),
                    color: [100, 149, 237],
                    output_device: None,
                },
                MixConfig {
                    name: "Stream".into(),
                    icon: "📡".into(),
                    color: [255, 99, 71],
                    output_device: None,
                },
            ],
            audio: AudioConfig {
                latency_ms: 20,
                output_device: "auto".into(),
            },
            ui: UiConfig {
                compact_mode: false,
                window_width: 1000,
                window_height: 600,
            },
        }
    }
}

impl AppConfig {
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

    pub fn save(&self) -> anyhow::Result<()> {
        confy::store("open-sound-grid", None, self)?;
        let path = confy::get_configuration_file_path("open-sound-grid", None)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        tracing::info!(path = %path, "config saved");
        Ok(())
    }
}
