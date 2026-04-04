//! Effects DSP processing functions for the PW filter process callback.
//!
//! Extracted from `filter.rs` for file-size compliance.
//! These are pure DSP functions with no PipeWire dependencies.

use super::filter::{CompressorParams, DeEsserParams, EnvelopeState, GateParams, LimiterParams};

/// Smart volume DSP parameters (RMS-based loudness normalization).
#[derive(Debug, Clone, Default)]
pub struct SmartVolumeParams {
    pub enabled: bool,
    pub target_db: f32,
    pub speed: f32,
    pub max_gain_db: f32,
}

/// Spatial audio DSP parameters (Bauer crossfeed + stereo width).
#[derive(Debug, Clone)]
pub struct SpatialAudioParams {
    pub enabled: bool,
    pub crossfeed: f32,
    pub width: f32,
}

impl Default for SpatialAudioParams {
    fn default() -> Self {
        Self {
            enabled: false,
            crossfeed: 0.3,
            width: 1.0,
        }
    }
}

/// Apply compressor to a buffer in-place. Returns peak after compression.
pub(super) fn apply_compressor(
    buf: &mut [f32],
    params: &CompressorParams,
    env: &mut EnvelopeState,
    sample_rate: f32,
) {
    if !params.enabled {
        return;
    }
    let threshold_lin = 10.0_f32.powf(params.threshold / 20.0);
    let makeup_lin = 10.0_f32.powf(params.makeup / 20.0);
    let attack_coeff = (-1.0 / (params.attack * sample_rate)).exp();
    let release_coeff = (-1.0 / (params.release * sample_rate)).exp();

    for s in buf.iter_mut() {
        let abs = s.abs();
        // Envelope follower
        let coeff = if abs > env.compressor_env {
            attack_coeff
        } else {
            release_coeff
        };
        env.compressor_env = coeff * env.compressor_env + (1.0 - coeff) * abs;

        // Gain computation
        if env.compressor_env > threshold_lin {
            let over_db = 20.0 * (env.compressor_env / threshold_lin).log10();
            let reduction_db = over_db * (1.0 - 1.0 / params.ratio);
            let gain = 10.0_f32.powf(-reduction_db / 20.0) * makeup_lin;
            *s *= gain;
        } else {
            *s *= makeup_lin;
        }
    }
}

/// Apply noise gate to a buffer in-place.
pub(super) fn apply_gate(
    buf: &mut [f32],
    params: &GateParams,
    env: &mut EnvelopeState,
    sample_rate: f32,
) {
    if !params.enabled {
        return;
    }
    let threshold_lin = 10.0_f32.powf(params.threshold / 20.0);
    let hold_samples = params.hold * sample_rate;
    let release_coeff = (-1.0 / (params.release * sample_rate)).exp();

    for s in buf.iter_mut() {
        let abs = s.abs();
        if abs > threshold_lin {
            env.gate_hold_counter = hold_samples;
            env.gate_env = 1.0; // open instantly (attack is very fast for gates)
        } else if env.gate_hold_counter > 0.0 {
            env.gate_hold_counter -= 1.0;
        } else {
            // Release phase
            let coeff = release_coeff;
            env.gate_env *= coeff;
        }
        *s *= env.gate_env;
    }
}

/// Apply de-esser to a buffer in-place (simplified: threshold-based gain reduction).
pub(super) fn apply_de_esser(buf: &mut [f32], params: &DeEsserParams) {
    if !params.enabled {
        return;
    }
    let threshold_lin = 10.0_f32.powf(params.threshold / 20.0);
    let max_reduction = 10.0_f32.powf(-params.reduction / 20.0);
    for s in buf.iter_mut() {
        let abs = s.abs();
        if abs > threshold_lin {
            let over = abs / threshold_lin;
            let reduction = (1.0 / over).max(max_reduction);
            *s *= reduction;
        }
    }
}

/// Apply volume boost (dB) to a buffer in-place.
pub(super) fn apply_boost(buf: &mut [f32], boost_db: f32) {
    if boost_db.abs() < 0.01 {
        return;
    }
    let gain = 10.0_f32.powf(boost_db / 20.0);
    for s in buf.iter_mut() {
        *s *= gain;
    }
}

/// Apply smart volume (RMS-based loudness normalization) to a buffer in-place.
///
/// Measures RMS over a ~400ms window and smoothly adjusts gain to match
/// the target level. Gain is clamped to `max_gain_db` to avoid boosting silence.
pub(super) fn apply_smart_volume(
    buf: &mut [f32],
    params: &SmartVolumeParams,
    env: &mut EnvelopeState,
    sample_rate: f32,
) {
    if !params.enabled {
        return;
    }

    // Initialize gain to unity on first call (Default gives 0.0).
    if env.sv_current_gain == 0.0 {
        env.sv_current_gain = 1.0;
    }

    let window_samples = (0.4 * sample_rate) as u32; // ~400ms window
    let max_gain_lin = 10.0_f32.powf(params.max_gain_db / 20.0);
    // Speed maps 0.0–1.0 to smoothing coefficient: slow (0.995) → fast (0.90)
    let smoothing = 1.0 - (params.speed.clamp(0.0, 1.0) * 0.095 + 0.005);

    for s in buf.iter_mut() {
        // Accumulate squared samples for RMS
        env.sv_rms_sum += (*s as f64) * (*s as f64);
        env.sv_sample_count += 1;

        // When window fills, compute new target gain
        if env.sv_sample_count >= window_samples {
            let rms = (env.sv_rms_sum / env.sv_sample_count as f64).sqrt() as f32;
            // Reset window
            env.sv_rms_sum = 0.0;
            env.sv_sample_count = 0;

            // Only adjust if signal is above noise floor (-60 dBFS)
            let noise_floor = 10.0_f32.powf(-60.0 / 20.0);
            if rms > noise_floor {
                let rms_db = 20.0 * rms.log10();
                let diff_db = params.target_db - rms_db;
                let target_gain = 10.0_f32.powf(diff_db / 20.0);
                // Clamp: never reduce below unity-minus-6dB, never exceed max_gain
                let target_gain = target_gain.clamp(10.0_f32.powf(-6.0 / 20.0), max_gain_lin);
                // Smooth toward target
                env.sv_current_gain =
                    smoothing * env.sv_current_gain + (1.0 - smoothing) * target_gain;
            }
            // If below noise floor, hold current gain (don't boost silence)
        }

        *s *= env.sv_current_gain;
    }
}

/// Apply brickwall limiter to a buffer in-place.
pub(super) fn apply_limiter(buf: &mut [f32], params: &LimiterParams) {
    if !params.enabled {
        return;
    }
    let ceiling_lin = 10.0_f32.powf(params.ceiling / 20.0);
    for s in buf.iter_mut() {
        if s.abs() > ceiling_lin {
            *s = s.signum() * ceiling_lin;
        }
    }
}

/// Apply Bauer crossfeed + stereo width to L/R buffer pair in-place.
///
/// Operates on both channels simultaneously — must be called after all
/// per-channel effects are complete. Crossfeed mixes a fraction of each
/// channel into the opposite to simulate speaker listening on headphones.
/// Width controls mid/side balance (1.0 = normal stereo).
pub(super) fn apply_spatial(buf_l: &mut [f32], buf_r: &mut [f32], params: &SpatialAudioParams) {
    if !params.enabled {
        return;
    }
    let cross = params.crossfeed * 0.5;
    let width = params.width;

    for i in 0..buf_l.len().min(buf_r.len()) {
        let l = buf_l[i];
        let r = buf_r[i];
        let mid = (l + r) * 0.5;
        let side = (l - r) * 0.5;
        buf_l[i] = (mid + side * width - r * cross).clamp(-1.0, 1.0);
        buf_r[i] = (mid - side * width - l * cross).clamp(-1.0, 1.0);
    }
}
