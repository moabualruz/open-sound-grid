//! Mixer preset management — save/load named mixer configurations.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::config::{ChannelConfig, MixConfig};

/// A complete mixer preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerPreset {
    pub name: String,
    pub channels: Vec<ChannelConfig>,
    pub mixes: Vec<MixConfig>,
    pub routes: Vec<PresetRoute>,
}

/// A single route in a preset (source_channel_name × mix_name → volume, enabled, muted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetRoute {
    pub channel_name: String,
    pub mix_name: String,
    pub volume: f32,
    pub enabled: bool,
    pub muted: bool,
}

impl MixerPreset {
    /// Build a preset from current config and engine state.
    #[instrument(skip(config, state))]
    pub fn from_current(name: &str, config: &crate::config::AppConfig, state: &crate::engine::state::MixerState) -> Self {
        let routes = state.routes.iter().map(|((source, mix_id), route)| {
            let channel_name = match source {
                crate::plugin::api::SourceId::Channel(id) => {
                    state.channels.iter().find(|c| c.id == *id)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("channel_{id}"))
                }
                crate::plugin::api::SourceId::Hardware(id) => format!("hw_{id}"),
                crate::plugin::api::SourceId::Mix(id) => format!("mix_{id}"),
            };
            let mix_name = state.mixes.iter().find(|m| m.id == *mix_id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| format!("mix_{mix_id}"));
            PresetRoute {
                channel_name,
                mix_name,
                volume: route.volume,
                enabled: route.enabled,
                muted: route.muted,
            }
        }).collect();

        Self {
            name: name.to_string(),
            channels: config.channels.clone(),
            mixes: config.mixes.clone(),
            routes,
        }
    }

    /// Save preset to disk.
    #[instrument(skip(self))]
    pub fn save(&self) -> anyhow::Result<PathBuf> {
        let dir = preset_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.toml", sanitize_filename(&self.name)));
        let toml = toml::to_string_pretty(self)?;
        fs::write(&path, toml)?;
        tracing::info!(path = %path.display(), name = %self.name, "preset saved");
        Ok(path)
    }

    /// Load a preset by name.
    #[instrument]
    pub fn load(name: &str) -> anyhow::Result<Self> {
        let path = preset_dir().join(format!("{}.toml", sanitize_filename(name)));
        let content = fs::read_to_string(&path)?;
        let preset: Self = toml::from_str(&content)?;
        tracing::info!(path = %path.display(), name = %preset.name, "preset loaded");
        Ok(preset)
    }

    /// List all saved preset names.
    #[instrument]
    pub fn list() -> Vec<String> {
        let dir = preset_dir();
        if !dir.exists() {
            return vec![];
        }
        let mut names = vec![];
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.path().file_stem() {
                    if entry.path().extension().map_or(false, |e| e == "toml") {
                        names.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
        names.sort();
        tracing::debug!(count = names.len(), "listed presets");
        names
    }

    /// Delete a preset by name.
    #[allow(dead_code)]
    pub fn delete(name: &str) -> anyhow::Result<()> {
        let path = preset_dir().join(format!("{}.toml", sanitize_filename(name)));
        fs::remove_file(&path)?;
        tracing::info!(name, "preset deleted");
        Ok(())
    }
}

fn preset_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "open-sound-grid")
        .map(|d| d.config_dir().join("presets"))
        .unwrap_or_else(|| PathBuf::from("~/.config/open-sound-grid/presets"))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Preset!"), "My_Preset_");
        assert_eq!(sanitize_filename("streaming-2024"), "streaming-2024");
    }

    #[test]
    fn test_preset_roundtrip() {
        let preset = MixerPreset {
            name: "Test".into(),
            channels: vec![ChannelConfig {
                name: "Music".into(),
                effects: Default::default(),
                muted: false,
            }],
            mixes: vec![MixConfig {
                name: "Monitor".into(),
                icon: String::new(),
                color: [128, 128, 128],
                output_device: None,
                master_volume: 1.0,
                muted: false,
            }],
            routes: vec![PresetRoute {
                channel_name: "Music".into(),
                mix_name: "Monitor".into(),
                volume: 0.8,
                enabled: true,
                muted: false,
            }],
        };
        let toml = toml::to_string_pretty(&preset).unwrap();
        let loaded: MixerPreset = toml::from_str(&toml).unwrap();
        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.channels.len(), 1);
        assert_eq!(loaded.routes[0].volume, 0.8);
    }
}
