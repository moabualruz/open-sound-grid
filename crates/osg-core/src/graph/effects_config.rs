// Effects chain configuration types — compressor, gate, de-esser, limiter,
// smart volume, spatial audio.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Effects — compressor / gate / de-esser / limiter configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressorConfig {
    pub enabled: bool,
    /// Threshold in dBFS (e.g., -18.0).
    pub threshold: f32,
    /// Compression ratio (e.g., 3.0 for 3:1).
    pub ratio: f32,
    /// Attack time in milliseconds (converted to seconds for DSP).
    pub attack: f32,
    /// Release time in milliseconds.
    pub release: f32,
    /// Make-up gain in dB.
    pub makeup: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -18.0,
            ratio: 3.0,
            attack: 8.0,
            release: 150.0,
            makeup: 4.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateConfig {
    pub enabled: bool,
    /// Threshold in dBFS (e.g., -45.0).
    pub threshold: f32,
    /// Hold time in milliseconds.
    pub hold: f32,
    /// Attack time in milliseconds.
    pub attack: f32,
    /// Release time in milliseconds.
    pub release: f32,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -45.0,
            hold: 150.0,
            attack: 1.0,
            release: 50.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeEsserConfig {
    pub enabled: bool,
    /// Center frequency in Hz (5000–8000).
    pub frequency: f32,
    /// Sidechain threshold in dBFS.
    pub threshold: f32,
    /// Maximum gain reduction in dB (positive, e.g., 6.0).
    pub reduction: f32,
}

impl Default for DeEsserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: 6000.0,
            threshold: -20.0,
            reduction: 6.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimiterConfig {
    pub enabled: bool,
    /// Output ceiling in dBFS (e.g., -1.0).
    pub ceiling: f32,
    /// Release time in milliseconds.
    pub release: f32,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling: -1.0,
            release: 50.0,
        }
    }
}

/// Smart volume (loudness normalization) configuration.
/// Measures short-term RMS loudness over a ~400ms window and applies
/// auto-gain to reach the target level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartVolumeConfig {
    pub enabled: bool,
    /// Target RMS level in dB (e.g., -18.0).
    pub target_db: f32,
    /// Response speed: how fast gain adjusts (0.0 = slow, 1.0 = fast).
    pub speed: f32,
    /// Maximum gain increase in dB (prevents boosting silence).
    pub max_gain_db: f32,
}

impl Default for SmartVolumeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target_db: -18.0,
            speed: 0.3,
            max_gain_db: 12.0,
        }
    }
}

/// Spatial audio configuration: Bauer crossfeed + stereo width for headphone listening.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialAudioConfig {
    pub enabled: bool,
    /// Crossfeed amount (0.0 = none, 1.0 = full mono).
    pub crossfeed: f32,
    /// Stereo width (0.0 = mono, 1.0 = normal, 2.0 = extra wide).
    pub width: f32,
}

impl Default for SpatialAudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            crossfeed: 0.3,
            width: 1.0,
        }
    }
}

/// Full effects chain configuration for a filter node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EffectsConfig {
    pub compressor: CompressorConfig,
    pub gate: GateConfig,
    pub de_esser: DeEsserConfig,
    pub limiter: LimiterConfig,
    /// Volume boost in dB (0–12). Applied as linear gain after limiter.
    #[serde(default)]
    pub boost: f32,
    /// Smart volume (loudness normalization).
    #[serde(default)]
    pub smart_volume: SmartVolumeConfig,
    /// Spatial audio: crossfeed + stereo width (mix-only, applied last).
    #[serde(default)]
    pub spatial: SpatialAudioConfig,
}
