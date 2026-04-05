//! Real-time FFT spectrum computation for audio analysis.
//!
//! Provides a pre-allocated ring buffer that accumulates samples on the RT
//! thread and, when full, computes a 1024-point real FFT to produce 256
//! log-scaled magnitude bins (20 Hz – 20 kHz).
//!
//! Thread model:
//! - `FftRingBuffer` lives in `CallbackData` (RT thread only, no sharing).
//! - Computed spectrum is published via `ArcSwap<SpectrumData>` on `FilterHandle`
//!   (lock-free read from any thread).
//! - FFT is only computed when the subscriber flag is set.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arc_swap::ArcSwap;
use realfft::{RealFftPlanner, RealToComplex};

use super::filter::filter_handle::SAMPLE_RATE;

/// Number of time-domain samples collected before computing FFT.
pub const FFT_SIZE: usize = 1024;

/// Number of magnitude bins in the output spectrum.
pub const SPECTRUM_BINS: usize = 256;

/// Minimum frequency for log-scaled bin mapping (Hz).
const FREQ_MIN: f32 = 20.0;

/// Maximum frequency for log-scaled bin mapping (Hz).
const FREQ_MAX: f32 = 20_000.0;
const SPECTRUM_FLOOR_DB: f32 = -100.0;

// ---------------------------------------------------------------------------
// Hann window (pre-computed at compile time is impractical, so lazy-init)
// ---------------------------------------------------------------------------

/// Compute Hann window coefficients for `FFT_SIZE` samples.
fn hann_window() -> [f32; FFT_SIZE] {
    let mut w = [0.0_f32; FFT_SIZE];
    let n = FFT_SIZE as f32;
    for (i, val) in w.iter_mut().enumerate() {
        let phase = 2.0 * std::f32::consts::PI * i as f32 / n;
        *val = 0.5 * (1.0 - phase.cos());
    }
    w
}

/// Pre-computed log-frequency bin edges mapping `SPECTRUM_BINS` output bins
/// to FFT frequency bins. Each entry is the (start_fft_bin, end_fft_bin) range.
fn log_bin_mapping() -> [(usize, usize); SPECTRUM_BINS] {
    let mut mapping = [(0usize, 0usize); SPECTRUM_BINS];
    let fft_bins = FFT_SIZE / 2 + 1;
    let bin_hz = SAMPLE_RATE / FFT_SIZE as f32;
    let log_min = FREQ_MIN.ln();
    let log_max = FREQ_MAX.ln();

    for (i, entry) in mapping.iter_mut().enumerate() {
        let t0 = i as f32 / SPECTRUM_BINS as f32;
        let t1 = (i + 1) as f32 / SPECTRUM_BINS as f32;
        let f0 = (log_min + t0 * (log_max - log_min)).exp();
        let f1 = (log_min + t1 * (log_max - log_min)).exp();
        let bin_start = if i == 0 {
            0 // Include DC and sub-20Hz in the first output bin
        } else {
            ((f0 / bin_hz).floor() as usize).min(fft_bins - 1)
        };
        let bin_end = (f1 / bin_hz).ceil() as usize;
        let bin_end = bin_end.clamp(bin_start + 1, fft_bins);
        *entry = (bin_start, bin_end);
    }
    mapping
}

// ---------------------------------------------------------------------------
// SpectrumData — published via ArcSwap
// ---------------------------------------------------------------------------

/// Immutable spectrum snapshot shared via `ArcSwap`.
#[derive(Debug, Clone)]
pub struct SpectrumData {
    /// Magnitude bins in dB (256 values).
    pub bins: [f32; SPECTRUM_BINS],
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            bins: [SPECTRUM_FLOOR_DB; SPECTRUM_BINS],
        }
    }
}

// ---------------------------------------------------------------------------
// FftRingBuffer — lives in CallbackData, RT-thread only
// ---------------------------------------------------------------------------

/// Pre-allocated ring buffer for accumulating samples and computing FFT.
/// One instance per channel (left/right) in `CallbackData`.
///
/// All memory is pre-allocated at construction time. No allocations occur
/// during `push_samples` or `compute_spectrum`.
pub struct FftRingBuffer {
    /// Circular sample buffer.
    buffer: [f32; FFT_SIZE],
    /// Current write position in the ring buffer.
    write_pos: usize,
    /// Number of samples accumulated since last FFT.
    samples_accumulated: usize,
    /// Pre-computed Hann window.
    window: [f32; FFT_SIZE],
    /// Pre-computed log-frequency bin mapping.
    bin_mapping: [(usize, usize); SPECTRUM_BINS],
    /// Pre-allocated FFT plan (avoids allocating planner on RT thread).
    fft_plan: Arc<dyn RealToComplex<f32>>,
    /// Scratch buffer for windowed time-domain data (input to FFT).
    scratch_input: Vec<f32>,
    /// Scratch buffer for FFT complex output.
    scratch_output: Vec<realfft::num_complex::Complex<f32>>,
}

impl std::fmt::Debug for FftRingBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FftRingBuffer")
            .field("write_pos", &self.write_pos)
            .field("samples_accumulated", &self.samples_accumulated)
            .finish()
    }
}

impl Default for FftRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl FftRingBuffer {
    /// Create a new ring buffer with all memory pre-allocated.
    pub fn new() -> Self {
        let output_len = FFT_SIZE / 2 + 1;
        // Pre-allocate planner and plan at construction (not on RT thread).
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_plan = planner.plan_fft_forward(FFT_SIZE);
        Self {
            buffer: [0.0; FFT_SIZE],
            write_pos: 0,
            samples_accumulated: 0,
            window: hann_window(),
            bin_mapping: log_bin_mapping(),
            fft_plan,
            scratch_input: vec![0.0; FFT_SIZE],
            scratch_output: vec![realfft::num_complex::Complex::new(0.0, 0.0); output_len],
        }
    }

    /// Push a single mono sample into the ring buffer.
    /// Returns `true` once `FFT_SIZE` samples have been accumulated.
    pub fn push_sample(&mut self, sample: f32) -> bool {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % FFT_SIZE;
        self.samples_accumulated = (self.samples_accumulated + 1).min(FFT_SIZE);
        self.samples_accumulated >= FFT_SIZE
    }

    /// Push audio samples into the ring buffer.
    /// Returns `true` when the buffer has accumulated `FFT_SIZE` samples
    /// and is ready for FFT computation.
    pub fn push_samples(&mut self, samples: &[f32]) -> bool {
        let mut ready = false;
        for &sample in samples {
            ready |= self.push_sample(sample);
        }
        ready
    }

    /// Compute the FFT spectrum and return magnitude bins in dB.
    /// Resets the accumulation counter. The ring buffer contents are preserved
    /// for overlap (new samples overwrite old ones naturally).
    ///
    /// Returns `None` if not enough samples have been accumulated.
    pub fn compute_spectrum(&mut self) -> Option<[f32; SPECTRUM_BINS]> {
        if self.samples_accumulated < FFT_SIZE {
            return None;
        }
        self.samples_accumulated = 0;

        // P1-3: Copy samples from ring buffer using two contiguous slices
        // instead of per-element modulo indexing.
        let start = self.write_pos; // oldest sample
        let first_len = FFT_SIZE - start;
        for i in 0..first_len {
            self.scratch_input[i] = self.buffer[start + i] * self.window[i];
        }
        for i in 0..start {
            self.scratch_input[first_len + i] = self.buffer[i] * self.window[first_len + i];
        }

        // Compute real FFT using pre-allocated plan (P0-1: no allocation on RT thread).
        // P1-1: process() overwrites scratch_output; no need to zero it first.
        if self
            .fft_plan
            .process(&mut self.scratch_input, &mut self.scratch_output)
            .is_err()
        {
            return None;
        }

        // Compute magnitudes and map to log-scaled bins
        let mut bins = [SPECTRUM_FLOOR_DB; SPECTRUM_BINS];
        let norm = 2.0 / FFT_SIZE as f32; // Normalization factor

        for (i, &(start_bin, end_bin)) in self.bin_mapping.iter().enumerate() {
            let mut max_mag: f32 = 0.0;
            for bin_idx in start_bin..end_bin {
                if let Some(c) = self.scratch_output.get(bin_idx) {
                    let mag = (c.re * c.re + c.im * c.im).sqrt() * norm;
                    if mag > max_mag {
                        max_mag = mag;
                    }
                }
            }
            // Convert to dB (with floor at -100 dB)
            bins[i] = if max_mag > 0.0 {
                (20.0 * max_mag.log10()).max(SPECTRUM_FLOOR_DB)
            } else {
                SPECTRUM_FLOOR_DB
            };
        }

        Some(bins)
    }
}

// ---------------------------------------------------------------------------
// Spectrum shared state (for FilterHandle)
// ---------------------------------------------------------------------------

/// Raw spectrum bins written by the RT thread (lock-free via atomics).
/// The WS broadcast thread reads these and wraps in `Arc<SpectrumData>`.
#[derive(Debug)]
struct RawSpectrumBins {
    bins: [std::sync::atomic::AtomicU32; SPECTRUM_BINS],
    /// Monotonic counter incremented on every publish; reader uses it to
    /// detect new data without allocating on the RT side.
    generation: std::sync::atomic::AtomicU64,
}

impl Default for RawSpectrumBins {
    fn default() -> Self {
        let floor_bits = SPECTRUM_FLOOR_DB.to_bits();
        Self {
            bins: std::array::from_fn(|_| std::sync::atomic::AtomicU32::new(floor_bits)),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

/// Shared spectrum state published by the RT thread and read by WebSocket.
#[derive(Debug, Clone)]
pub struct SpectrumHandle {
    /// Whether any WebSocket client is subscribed to this node's spectrum.
    pub subscribed: Arc<AtomicBool>,
    /// Raw bins written by RT thread (no allocation).
    raw: Arc<RawSpectrumBins>,
    /// Cached Arc for the reader side (WS handler does the Arc::new).
    cached: Arc<ArcSwap<SpectrumData>>,
    /// Last generation the reader saw.
    reader_gen: Arc<std::sync::atomic::AtomicU64>,
}

impl Default for SpectrumHandle {
    fn default() -> Self {
        Self {
            subscribed: Arc::new(AtomicBool::new(false)),
            raw: Arc::new(RawSpectrumBins::default()),
            cached: Arc::new(ArcSwap::new(Arc::new(SpectrumData::default()))),
            reader_gen: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

impl SpectrumHandle {
    /// Check if any client is subscribed (RT-safe, lock-free).
    pub fn is_subscribed(&self) -> bool {
        self.subscribed.load(Ordering::Relaxed)
    }

    /// Set subscription state (called from WebSocket handler).
    pub fn set_subscribed(&self, subscribed: bool) {
        self.subscribed.store(subscribed, Ordering::Relaxed);
    }

    /// Check whether any spectrum has been published yet.
    pub fn has_published_data(&self) -> bool {
        self.raw.generation.load(Ordering::Acquire) > 0
    }

    /// Publish new spectrum data (called from RT thread).
    ///
    /// Writes raw f32 bins via atomic stores — zero allocation, lock-free.
    /// The reader side (`load`) materialises the `Arc<SpectrumData>`.
    pub fn publish(&self, data: SpectrumData) {
        for (i, &value) in data.bins.iter().enumerate() {
            self.raw.bins[i].store(value.to_bits(), Ordering::Relaxed);
        }
        // Release fence so the reader sees all bin writes before the new generation.
        self.raw.generation.fetch_add(1, Ordering::Release);
    }

    /// Load current spectrum data (called from WebSocket handler, lock-free).
    ///
    /// Allocates a new `Arc<SpectrumData>` only when new data is available,
    /// keeping the allocation off the RT thread.
    pub fn load(&self) -> Arc<SpectrumData> {
        let current_gen = self.raw.generation.load(Ordering::Acquire);
        let last_gen = self.reader_gen.load(Ordering::Relaxed);
        if current_gen != last_gen {
            let mut sd = SpectrumData::default();
            for (i, atom) in self.raw.bins.iter().enumerate() {
                sd.bins[i] = f32::from_bits(atom.load(Ordering::Relaxed));
            }
            let arc = Arc::new(sd);
            self.cached.store(arc.clone());
            self.reader_gen.store(current_gen, Ordering::Relaxed);
            arc
        } else {
            self.cached.load_full()
        }
    }
}
