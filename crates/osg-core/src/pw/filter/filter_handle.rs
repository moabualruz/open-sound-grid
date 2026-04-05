//! FilterHandle and all EQ/effects parameter types.
//!
//! Extracted from `filter.rs` for file-size compliance.
//! These types are shared between the PW mainloop thread (RT callback)
//! and the reducer/main thread (parameter updates, peak reads).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use arc_swap::ArcSwap;

use crate::graph::EqConfig;
use crate::pw::biquad::{Coefficients, compute_coefficients};
use crate::pw::fft::SpectrumHandle;

pub(crate) const SAMPLE_RATE: f32 = 48000.0;
pub(crate) const MAX_BANDS: usize = 10;
/// Max macro bands (Bass, Voice, Treble) — separate from user's 10 bands.
/// Used by the frontend; kept here for parity with MAX_BANDS.
pub const MAX_MACRO_BANDS: usize = 3;

// ---------------------------------------------------------------------------
// RT-safe peak packing helpers
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

// ---------------------------------------------------------------------------
// CompiledEq
// ---------------------------------------------------------------------------

/// Pre-computed biquad coefficients for all bands.
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

// ---------------------------------------------------------------------------
// Effects DSP params
// ---------------------------------------------------------------------------

/// Compressor parameters. Operates on per-sample envelope.
#[derive(Debug, Clone)]
pub struct CompressorParams {
    pub enabled: bool,
    pub threshold: f32, // dB (negative, e.g., -18.0)
    pub ratio: f32,     // e.g., 3.0 for 3:1
    pub attack: f32,    // seconds
    pub release: f32,   // seconds
    pub makeup: f32,    // dB
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -18.0,
            ratio: 3.0,
            attack: 0.008,
            release: 0.15,
            makeup: 4.0,
        }
    }
}

/// Noise gate parameters.
#[derive(Debug, Clone)]
pub struct GateParams {
    pub enabled: bool,
    pub threshold: f32, // dB (e.g., -45.0)
    pub hold: f32,      // seconds
    pub attack: f32,    // seconds
    pub release: f32,   // seconds
}

impl Default for GateParams {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -45.0,
            hold: 0.15,
            attack: 0.001,
            release: 0.05,
        }
    }
}

/// De-esser parameters — simple sidechain bandpass + gain reduction.
#[derive(Debug, Clone)]
pub struct DeEsserParams {
    pub enabled: bool,
    pub frequency: f32, // center Hz (5000-8000)
    pub threshold: f32, // dB
    pub reduction: f32, // max reduction in dB (positive, e.g., 6.0)
}

impl Default for DeEsserParams {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: 6000.0,
            threshold: -20.0,
            reduction: 6.0,
        }
    }
}

/// Limiter parameters — brickwall peak limiter.
#[derive(Debug, Clone)]
pub struct LimiterParams {
    pub enabled: bool,
    pub ceiling: f32, // dBFS (e.g., -1.0)
    pub release: f32, // seconds
}

impl Default for LimiterParams {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling: -1.0,
            release: 0.05,
        }
    }
}

// SmartVolumeParams and SpatialAudioParams live in effects_dsp.rs.
pub use crate::pw::effects_dsp::{SmartVolumeParams, SpatialAudioParams};

/// All effects params bundled for ArcSwap sharing.
#[derive(Debug, Clone, Default)]
pub struct EffectsParams {
    pub compressor: CompressorParams,
    pub gate: GateParams,
    pub de_esser: DeEsserParams,
    pub limiter: LimiterParams,
    /// Volume boost in dB (0–12). Applied as linear gain after limiter.
    pub boost: f32,
    pub smart_volume: SmartVolumeParams,
    pub spatial: SpatialAudioParams,
}

/// Per-channel envelope state for compressor/gate/smart-volume (lives in CallbackData, not shared).
#[derive(Debug, Default)]
pub(crate) struct EnvelopeState {
    pub(crate) compressor_env: f32,    // current envelope level (linear)
    pub(crate) gate_env: f32,          // gate envelope (0.0 = closed, 1.0 = open)
    pub(crate) gate_hold_counter: f32, // samples remaining in hold phase
    pub(crate) sv_rms_sum: f64,        // running sum of squared samples for RMS window
    pub(crate) sv_sample_count: u32,   // samples accumulated in current RMS window
    pub(crate) sv_current_gain: f32,   // current applied gain (linear, starts at 1.0)
}

// ---------------------------------------------------------------------------
// FilterHandle
// ---------------------------------------------------------------------------

/// Shared handle for lock-free EQ/volume/mute parameter passing and peak reading.
#[derive(Clone, Debug)]
pub struct FilterHandle {
    pub(super) eq: Arc<ArcSwap<CompiledEq>>,
    pub(super) effects: Arc<ArcSwap<EffectsParams>>,
    pub(super) peaks: Arc<AtomicU64>,
    pub(super) volume_left: Arc<AtomicU32>,
    pub(super) volume_right: Arc<AtomicU32>,
    pub(super) muted: Arc<AtomicBool>,
    /// When true, filter passes audio through without EQ processing.
    /// Always-resident filters start bypassed; enabling EQ clears this flag.
    pub(super) bypassed: Arc<AtomicBool>,
    /// FFT spectrum data shared between RT thread and WebSocket handler.
    pub(crate) spectrum: SpectrumHandle,
}

impl Default for FilterHandle {
    fn default() -> Self {
        Self {
            eq: Arc::new(ArcSwap::new(Arc::new(CompiledEq::empty()))),
            effects: Arc::new(ArcSwap::new(Arc::new(EffectsParams::default()))),
            peaks: Arc::new(AtomicU64::new(0)),
            volume_left: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            volume_right: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            muted: Arc::new(AtomicBool::new(false)),
            bypassed: Arc::new(AtomicBool::new(true)),
            spectrum: SpectrumHandle::default(),
        }
    }
}

impl FilterHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set bypass state (lock-free, any thread). Bypassed = passthrough.
    pub fn set_bypassed(&self, bypassed: bool) {
        self.bypassed.store(bypassed, Ordering::Release);
    }

    /// Read bypass state (lock-free).
    pub fn is_bypassed(&self) -> bool {
        self.bypassed.load(Ordering::Acquire)
    }

    /// Update EQ parameters and clear bypass (lock-free, any thread).
    pub fn set_eq(&self, config: &EqConfig) {
        self.eq.store(Arc::new(CompiledEq::from_config(config)));
        if !config.bands.is_empty() {
            self.bypassed.store(false, Ordering::Release);
        }
    }

    /// Update effects parameters (lock-free, any thread).
    pub fn set_effects(&self, params: EffectsParams) {
        self.effects.store(Arc::new(params));
    }

    /// Load current effects params (for RT callback).
    pub fn load_effects(&self) -> arc_swap::Guard<Arc<EffectsParams>> {
        self.effects.load()
    }

    /// Set stereo volume (lock-free, any thread). Values 0.0–1.0.
    pub fn set_volume(&self, left: f32, right: f32) {
        let left = left.clamp(0.0, 1.5);
        let right = right.clamp(0.0, 1.5);
        self.volume_left.store(left.to_bits(), Ordering::Relaxed);
        self.volume_right.store(right.to_bits(), Ordering::Relaxed);
    }

    /// Set mute state (lock-free, any thread).
    pub fn set_mute(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Read current volume (lock-free).
    pub fn volume(&self) -> (f32, f32) {
        (
            f32::from_bits(self.volume_left.load(Ordering::Relaxed)),
            f32::from_bits(self.volume_right.load(Ordering::Relaxed)),
        )
    }

    /// Read current mute state (lock-free).
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Read latest peak levels (lock-free).
    pub fn peak(&self) -> (f32, f32) {
        unpack_peaks(self.peaks.load(Ordering::Relaxed))
    }

    /// Load current compiled EQ (for RT callback).
    pub fn load_eq(&self) -> arc_swap::Guard<Arc<CompiledEq>> {
        self.eq.load()
    }

    /// Store peak values (for RT callback).
    pub fn store_peaks(&self, left: f32, right: f32) {
        self.peaks.store(pack_peaks(left, right), Ordering::Relaxed);
    }

    /// Get the spectrum handle for FFT subscription and data access.
    pub fn spectrum(&self) -> &SpectrumHandle {
        &self.spectrum
    }
}
