//! Safe wrapper around `pw_filter` for inline DSP processing.
//!
//! This module will isolate ALL unsafe PipeWire FFI behind a safe API.
//! The `OsgFilter` struct creates a PW filter node with stereo in/out,
//! applies biquad EQ in the process callback, and reports peak levels.
//!
//! Thread model:
//! - `OsgFilter` is created and owned on the PW mainloop thread.
//! - The `process` callback runs on the PW real-time thread.
//! - EQ parameters cross from main → RT via `ArcSwap` (lock-free).
//! - Peak levels cross from RT → main via packed `AtomicU64`.
//!
//! # Status
//! Phase 2b skeleton — public API defined, FFI implementation pending.
//! The actual `pw_filter` calls require `#![allow(unsafe_code)]` and
//! careful lifetime management of the callback data.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::graph::EqConfig;
use crate::pw::biquad::{BiquadState, Coefficients, compute_coefficients};

const SAMPLE_RATE: f32 = 48000.0;
const MAX_BANDS: usize = 10;

// ---------------------------------------------------------------------------
// RT-safe shared state
// ---------------------------------------------------------------------------

/// Pack two f32 peak values into a single atomic u64.
pub fn pack_peaks(left: f32, right: f32) -> u64 {
    let l = left.to_bits() as u64;
    let r = (right.to_bits() as u64) << 32;
    l | r
}

/// Unpack two f32 peak values from a single atomic u64.
pub fn unpack_peaks(packed: u64) -> (f32, f32) {
    let l = f32::from_bits(packed as u32);
    let r = f32::from_bits((packed >> 32) as u32);
    (l, r)
}

/// Pre-computed biquad coefficients for all bands.
/// Avoids recomputing per sample in the RT callback.
#[derive(Debug)]
pub struct CompiledEq {
    pub bands: Vec<(Coefficients, bool)>,
}

impl CompiledEq {
    pub fn from_config(config: &EqConfig) -> Self {
        let bands = config
            .bands
            .iter()
            .take(MAX_BANDS)
            .map(|b| {
                let coeffs =
                    compute_coefficients(b.filter_type, b.frequency, b.gain, b.q, SAMPLE_RATE);
                (coeffs, b.enabled && config.enabled)
            })
            .collect();
        Self { bands }
    }

    pub fn empty() -> Self {
        Self { bands: Vec::new() }
    }
}

/// Shared state handle for lock-free EQ parameter passing and peak reading.
/// Created by OsgFilter, used by the mainloop to set EQ and read peaks.
#[derive(Clone)]
pub struct FilterHandle {
    eq: Arc<ArcSwap<CompiledEq>>,
    peaks: Arc<AtomicU64>,
}

impl FilterHandle {
    /// Create a new handle (used internally by OsgFilter).
    pub fn new() -> Self {
        Self {
            eq: Arc::new(ArcSwap::new(Arc::new(CompiledEq::empty()))),
            peaks: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Update EQ parameters (lock-free, safe to call from any thread).
    pub fn set_eq(&self, config: &EqConfig) {
        let compiled = Arc::new(CompiledEq::from_config(config));
        self.eq.store(compiled);
    }

    /// Read the latest peak levels (lock-free).
    pub fn peak(&self) -> (f32, f32) {
        unpack_peaks(self.peaks.load(Ordering::Relaxed))
    }

    /// Load the current compiled EQ (for RT callback use).
    pub fn load_eq(&self) -> arc_swap::Guard<Arc<CompiledEq>> {
        self.eq.load()
    }

    /// Store peak values (for RT callback use).
    pub fn store_peaks(&self, left: f32, right: f32) {
        self.peaks.store(pack_peaks(left, right), Ordering::Relaxed);
    }
}

/// Process a block of mono audio samples through the EQ cascade.
/// Called from the RT process callback.
pub fn process_block(
    input: &[f32],
    output: &mut [f32],
    eq: &CompiledEq,
    states: &mut [BiquadState],
) -> f32 {
    let mut peak: f32 = 0.0;
    for (i, &sample) in input.iter().enumerate() {
        let mut s = sample;
        for (band_idx, (coeffs, enabled)) in eq.bands.iter().enumerate() {
            if *enabled {
                if let Some(state) = states.get_mut(band_idx) {
                    s = state.process(s, coeffs);
                }
            }
        }
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
        output[i] = s;
    }
    peak
}
