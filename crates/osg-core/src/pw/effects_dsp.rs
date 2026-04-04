//! Effects DSP processing functions for the PW filter process callback.
//!
//! Extracted from `filter.rs` for file-size compliance.
//! These are pure DSP functions with no PipeWire dependencies.

use super::filter::{CompressorParams, DeEsserParams, EnvelopeState, GateParams, LimiterParams};

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
