// Parametric EQ configuration types.

use serde::{Deserialize, Serialize};

/// Biquad filter type for a single EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterType {
    Peaking,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    Notch,
}

/// A single parametric EQ band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBand {
    pub enabled: bool,
    pub filter_type: FilterType,
    /// Center frequency in Hz (20–20 000).
    pub frequency: f32,
    /// Gain in dB (±12).
    pub gain: f32,
    /// Quality factor (0.1–10).
    pub q: f32,
}

impl Default for EqBand {
    fn default() -> Self {
        Self {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 0.0,
            q: 0.707,
        }
    }
}

/// Full EQ configuration: enable toggle + ordered list of bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqConfig {
    pub enabled: bool,
    pub bands: Vec<EqBand>,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bands: Vec::new(),
        }
    }
}
