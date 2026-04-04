// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// Configuration persistence using TOML. State and settings are saved to
// XDG-compliant directories (overridable via env vars).

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

use crate::graph::{MixerSession, ReconcileSettings};
use crate::migration;

const APP_ID: &str = "open-sound-grid";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Errors originating from configuration persistence.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("could not resolve data directory")]
    DataDirNotFound,

    #[error("could not resolve config directory")]
    ConfigDirNotFound,

    #[error("failed to serialize: {0}")]
    SerializeFailed(#[from] toml::ser::Error),

    #[error("failed to deserialize: {0}")]
    DeserializeFailed(#[from] toml::de::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Directory resolution
// ---------------------------------------------------------------------------

fn data_dir() -> Option<PathBuf> {
    std::env::var("OSG_DATA_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(dirs::data_local_dir)
}

fn config_dir() -> Option<PathBuf> {
    std::env::var("OSG_CONFIG_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(dirs::config_local_dir)
}

// ---------------------------------------------------------------------------
// PersistentState — the desired-state snapshot saved to disk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    pub(crate) version: String,
    pub(crate) state: MixerSession,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            version: migration::CURRENT_VERSION.to_owned(),
            state: MixerSession::default(),
        }
    }
}

impl PersistentState {
    /// Build a saveable snapshot, stripping transient data.
    pub fn from_state(mut state: MixerSession, runtime: &crate::graph::RuntimeState) -> Self {
        // Only persist locked links.
        state.links.retain(|link| link.state.is_locked());
        // Strip transient cell_node_id from links before persistence.
        for link in &mut state.links {
            link.cell_node_id = None;
        }
        // Only persist active applications.
        state.apps.retain(|id, _| runtime.app_is_active(id));

        Self {
            version: migration::CURRENT_VERSION.to_owned(),
            state,
        }
    }

    pub fn into_state(self) -> MixerSession {
        self.state
    }

    pub fn save(&mut self) -> Result<(), ConfigError> {
        // Always stamp the current migration version before writing.
        self.version = migration::CURRENT_VERSION.to_owned();

        let dir = data_dir().ok_or(ConfigError::DataDirNotFound)?;
        let app_dir = dir.join(APP_ID);
        fs::create_dir_all(&app_dir)?;

        let path = app_dir.join("state.toml");
        let content = toml::to_string_pretty(self)?;
        let mut file = File::create(&path)?;
        file.write_all(content.as_bytes())?;
        debug!("[Config] saved state to {}", path.display());
        Ok(())
    }

    pub fn load() -> Result<Self, ConfigError> {
        let dir = data_dir().ok_or(ConfigError::DataDirNotFound)?;
        let path = dir.join(APP_ID).join("state.toml");
        let content = fs::read_to_string(&path)?;
        migration::migrate(&content)
    }
}

// ---------------------------------------------------------------------------
// PersistentSettings — reconciliation settings saved to disk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentSettings {
    version: String,
    settings: ReconcileSettings,
}

impl PersistentSettings {
    pub fn from_settings(settings: ReconcileSettings) -> Self {
        Self {
            version: APP_VERSION.to_string(),
            settings,
        }
    }

    pub fn into_settings(self) -> ReconcileSettings {
        self.settings
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let dir = config_dir().ok_or(ConfigError::ConfigDirNotFound)?;
        let app_dir = dir.join(APP_ID);
        fs::create_dir_all(&app_dir)?;

        let path = app_dir.join("settings.toml");
        let content = toml::to_string_pretty(self)?;
        let mut file = File::create(&path)?;
        file.write_all(content.as_bytes())?;
        debug!("[Config] saved settings to {}", path.display());
        Ok(())
    }

    pub fn load() -> Result<Self, ConfigError> {
        let dir = config_dir().ok_or(ConfigError::ConfigDirNotFound)?;
        let path = dir.join(APP_ID).join("settings.toml");
        let content = fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }
}
