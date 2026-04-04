// State migration framework for Open Sound Grid.
//
// Handles schema evolution of PersistentState between versions.
// When the TOML on disk was written by an older (or newer) binary,
// this module applies sequential migrations so the current code can
// load it safely.

use crate::config::{ConfigError, PersistentState};

/// Current state format version. Bump when PersistentState schema changes.
pub const CURRENT_VERSION: &str = "0.2.0";

/// Previous versions that have a known migration path, in order.
const KNOWN_VERSIONS: &[&str] = &["0.1.0"];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Migrate raw TOML content to the current `PersistentState` format.
///
/// Strategy:
/// 1. Parse as `toml::Value` to read the version field.
/// 2. If version matches `CURRENT_VERSION`, deserialize directly.
/// 3. If version is an older known version, apply sequential migrations.
/// 4. If TOML is corrupt, missing, or has an unknown version, return a
///    default `PersistentState` with a warning log.
pub fn migrate(raw: &str) -> Result<PersistentState, ConfigError> {
    // Step 1: parse as generic TOML table to inspect the version.
    // NOTE: `toml::from_str` (serde path), NOT `str::parse::<Value>()` — the
    // `FromStr` impl in toml 1.x rejects valid TOML that the serde path accepts.
    let mut table: toml::Value = match toml::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("[Migration] state file is not valid TOML, returning defaults: {e}");
            return Ok(PersistentState::default());
        }
    };

    let version = table
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();

    // Step 2: current version — deserialize directly from original string.
    if version == CURRENT_VERSION {
        return deserialize_str(raw);
    }

    // Step 3: known older version — apply migrations in sequence.
    if let Some(start) = KNOWN_VERSIONS.iter().position(|v| *v == version) {
        for (i, from_version) in KNOWN_VERSIONS[start..].iter().enumerate() {
            let to_version = KNOWN_VERSIONS
                .get(start + i + 1)
                .copied()
                .unwrap_or(CURRENT_VERSION);

            tracing::info!("[Migration] migrating state from {from_version} to {to_version}");
            apply_migration(&mut table, from_version);
        }

        // Stamp current version after all migrations.
        if let Some(t) = table.as_table_mut() {
            t.insert(
                "version".to_owned(),
                toml::Value::String(CURRENT_VERSION.to_owned()),
            );
        }

        return deserialize_value(table);
    }

    // Step 4: unknown or newer version — fall back to defaults.
    tracing::warn!("[Migration] unknown state version \"{version}\", returning defaults");
    Ok(PersistentState::default())
}

// ---------------------------------------------------------------------------
// Per-version migration functions
// ---------------------------------------------------------------------------

/// Apply a single migration step from `from_version` to the next version.
///
/// Each migration mutates the `toml::Value` tree in-place. Adding new
/// migration steps is the primary extension point:
///
/// ```text
/// "0.1.0" → add missing `effects` tables with defaults
/// "0.2.0" → (next schema change goes here)
/// ```
fn apply_migration(table: &mut toml::Value, from_version: &str) {
    match from_version {
        "0.1.0" => migrate_0_1_to_0_2(table),
        other => {
            tracing::warn!("[Migration] no migration function for version {other}");
        }
    }
}

/// 0.1.0 → 0.2.0: ensure effects/eq fields exist on endpoints and links.
///
/// In 0.1.0 these fields did not exist. The `serde(default)` attributes
/// handle missing fields during deserialization, so this migration is a
/// no-op structurally — it exists as a template and to log the transition.
fn migrate_0_1_to_0_2(_table: &mut toml::Value) {
    // serde(default) on EqConfig, EffectsConfig, and new MixerSession
    // fields handles missing data automatically. No manual patching needed.
    tracing::debug!("[Migration] 0.1.0 → 0.2.0: serde defaults handle new fields");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deserialize directly from TOML string. Used for current-version files
/// where the original string preserves all key casing and structure.
fn deserialize_str(raw: &str) -> Result<PersistentState, ConfigError> {
    match toml::from_str::<PersistentState>(raw) {
        Ok(state) => Ok(state),
        Err(e) => {
            tracing::warn!(
                "[Migration] failed to deserialize current-version state, returning defaults: {e}"
            );
            Ok(PersistentState::default())
        }
    }
}

/// Deserialize a `toml::Value` (post-migration) into `PersistentState`.
/// Re-serializes to string first so that serde attributes (`rename_all`,
/// custom deserializers, `serde(default)`) work correctly.
fn deserialize_value(value: toml::Value) -> Result<PersistentState, ConfigError> {
    let toml_str = match toml::to_string(&value) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[Migration] failed to re-serialize migrated state: {e}");
            return Ok(PersistentState::default());
        }
    };
    deserialize_str(&toml_str)
}
