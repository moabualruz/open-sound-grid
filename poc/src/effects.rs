//! Per-channel audio effects chain using fundsp.
//!
//! Each channel can have: parametric EQ → compressor → noise gate.
//! Effects parameters are stored per-channel and synchronized to the plugin.
//! Actual audio processing is wired when PA stream capture is available.

use serde::{Deserialize, Serialize};

/// Parameters for a single channel's effects chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsParams {
    /// Parametric EQ center frequency (Hz)
    pub eq_freq_hz: f32,
    /// Parametric EQ Q factor (0.1 - 10.0)
    pub eq_q: f32,
    /// Parametric EQ gain (dB, -24.0 to +24.0)
    pub eq_gain_db: f32,
    /// Compressor threshold (dB, -60.0 to 0.0)
    pub comp_threshold_db: f32,
    /// Compressor ratio (1.0 to 20.0)
    pub comp_ratio: f32,
    /// Compressor attack time (ms)
    pub comp_attack_ms: f32,
    /// Compressor release time (ms)
    pub comp_release_ms: f32,
    /// Noise gate threshold (dB, -80.0 to 0.0)
    pub gate_threshold_db: f32,
    /// Noise gate hold time (ms)
    pub gate_hold_ms: f32,
    /// Whether the effects chain is active
    pub enabled: bool,
}

impl Default for EffectsParams {
    fn default() -> Self {
        Self {
            eq_freq_hz: 1000.0,
            eq_q: 1.0,
            eq_gain_db: 0.0,
            comp_threshold_db: -20.0,
            comp_ratio: 4.0,
            comp_attack_ms: 10.0,
            comp_release_ms: 100.0,
            gate_threshold_db: -60.0,
            gate_hold_ms: 50.0,
            enabled: false,
        }
    }
}

impl PartialEq for EffectsParams {
    fn eq(&self, other: &Self) -> bool {
        self.eq_freq_hz == other.eq_freq_hz
            && self.eq_q == other.eq_q
            && self.eq_gain_db == other.eq_gain_db
            && self.comp_threshold_db == other.comp_threshold_db
            && self.comp_ratio == other.comp_ratio
            && self.comp_attack_ms == other.comp_attack_ms
            && self.comp_release_ms == other.comp_release_ms
            && self.gate_threshold_db == other.gate_threshold_db
            && self.gate_hold_ms == other.gate_hold_ms
            && self.enabled == other.enabled
    }
}

/// An effects chain that can process audio buffers.
/// Uses fundsp's AudioUnit for dynamic graph construction.
pub struct EffectsChain {
    params: EffectsParams,
    // fundsp graph will be constructed here when audio processing is wired.
    // For now, just store parameters.
}

impl EffectsChain {
    pub fn new() -> Self {
        tracing::debug!("creating new effects chain with default params");
        Self {
            params: EffectsParams::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_params(params: EffectsParams) -> Self {
        tracing::debug!(params = ?params, "creating effects chain with custom params");
        Self { params }
    }

    #[allow(dead_code)]
    pub fn params(&self) -> &EffectsParams {
        &self.params
    }

    pub fn set_params(&mut self, params: EffectsParams) {
        tracing::debug!(
            eq_freq = params.eq_freq_hz,
            eq_gain = params.eq_gain_db,
            comp_threshold = params.comp_threshold_db,
            gate_threshold = params.gate_threshold_db,
            enabled = params.enabled,
            "updating effects params"
        );
        self.params = params;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        tracing::debug!(enabled, "effects chain enabled state changed");
        self.params.enabled = enabled;
    }

    /// Process a buffer of f32 samples through the effects chain.
    /// Currently a no-op passthrough — actual processing wired when PA streams available.
    #[allow(dead_code)]
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        // TODO: Wire fundsp processing when PA stream capture is available.
        // For now, passthrough.
        output.copy_from_slice(input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_params() {
        let params = EffectsParams::default();
        assert_eq!(params.eq_freq_hz, 1000.0);
        assert!(!params.enabled);
    }

    #[test]
    fn test_effects_chain_passthrough() {
        let mut chain = EffectsChain::new();
        let input = vec![0.5f32, -0.3, 0.8, -0.1];
        let mut output = vec![0.0f32; 4];
        chain.process(&input, &mut output);
        assert_eq!(input, output);
    }

    #[test]
    fn test_params_roundtrip() {
        let params = EffectsParams {
            eq_freq_hz: 2000.0,
            comp_ratio: 8.0,
            enabled: true,
            ..Default::default()
        };
        let toml_str = toml::to_string(&params).unwrap();
        let loaded: EffectsParams = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.eq_freq_hz, 2000.0);
        assert_eq!(loaded.comp_ratio, 8.0);
        assert!(loaded.enabled);
    }
}
