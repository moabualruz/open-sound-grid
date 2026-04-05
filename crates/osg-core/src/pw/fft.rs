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
use realfft::RealFftPlanner;

use super::filter::filter_handle::SAMPLE_RATE;

/// Number of time-domain samples collected before computing FFT.
pub const FFT_SIZE: usize = 1024;

/// Number of magnitude bins in the output spectrum.
pub const SPECTRUM_BINS: usize = 256;

/// Minimum frequency for log-scaled bin mapping (Hz).
const FREQ_MIN: f32 = 20.0;

/// Maximum frequency for log-scaled bin mapping (Hz).
const FREQ_MAX: f32 = 20_000.0;

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
    /// Left channel magnitude bins in dB (256 values).
    pub left: [f32; SPECTRUM_BINS],
    /// Right channel magnitude bins in dB (256 values).
    pub right: [f32; SPECTRUM_BINS],
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            left: [f32::NEG_INFINITY; SPECTRUM_BINS],
            right: [f32::NEG_INFINITY; SPECTRUM_BINS],
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
        Self {
            buffer: [0.0; FFT_SIZE],
            write_pos: 0,
            samples_accumulated: 0,
            window: hann_window(),
            bin_mapping: log_bin_mapping(),
            scratch_input: vec![0.0; FFT_SIZE],
            scratch_output: vec![realfft::num_complex::Complex::new(0.0, 0.0); output_len],
        }
    }

    /// Push audio samples into the ring buffer.
    /// Returns `true` when the buffer has accumulated `FFT_SIZE` samples
    /// and is ready for FFT computation.
    pub fn push_samples(&mut self, samples: &[f32]) -> bool {
        for &s in samples {
            self.buffer[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % FFT_SIZE;
            self.samples_accumulated += 1;
        }
        self.samples_accumulated >= FFT_SIZE
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

        // Copy samples from ring buffer in order, applying Hann window
        let start = self.write_pos; // oldest sample
        for i in 0..FFT_SIZE {
            let idx = (start + i) % FFT_SIZE;
            self.scratch_input[i] = self.buffer[idx] * self.window[i];
        }

        // Compute real FFT
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        // Reset output buffer
        for c in &mut self.scratch_output {
            *c = realfft::num_complex::Complex::new(0.0, 0.0);
        }
        if fft
            .process(&mut self.scratch_input, &mut self.scratch_output)
            .is_err()
        {
            return None;
        }

        // Compute magnitudes and map to log-scaled bins
        let mut bins = [f32::NEG_INFINITY; SPECTRUM_BINS];
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
                (20.0 * max_mag.log10()).max(-100.0)
            } else {
                -100.0
            };
        }

        Some(bins)
    }
}

// ---------------------------------------------------------------------------
// Spectrum shared state (for FilterHandle)
// ---------------------------------------------------------------------------

/// Shared spectrum state published by the RT thread and read by WebSocket.
#[derive(Debug, Clone)]
pub struct SpectrumHandle {
    /// Whether any WebSocket client is subscribed to this node's spectrum.
    pub subscribed: Arc<AtomicBool>,
    /// Latest computed spectrum data.
    pub data: Arc<ArcSwap<SpectrumData>>,
}

impl Default for SpectrumHandle {
    fn default() -> Self {
        Self {
            subscribed: Arc::new(AtomicBool::new(false)),
            data: Arc::new(ArcSwap::new(Arc::new(SpectrumData::default()))),
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

    /// Publish new spectrum data (called from RT thread, lock-free).
    pub fn publish(&self, data: SpectrumData) {
        self.data.store(Arc::new(data));
    }

    /// Load current spectrum data (called from WebSocket handler, lock-free).
    pub fn load(&self) -> Arc<SpectrumData> {
        self.data.load_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_dc_signal_energy_in_low_bins() {
        let mut rb = FftRingBuffer::new();
        let dc_signal = [0.5_f32; FFT_SIZE];
        assert!(rb.push_samples(&dc_signal));
        let bins = rb.compute_spectrum().expect("should produce spectrum");
        // DC energy should be concentrated in the low-frequency bins.
        // Hann window spreads energy slightly, so check that average of
        // bottom quarter is well above average of top quarter.
        let low_avg: f32 = bins[..SPECTRUM_BINS / 4].iter().sum::<f32>() / (SPECTRUM_BINS / 4) as f32;
        let high_avg: f32 = bins[SPECTRUM_BINS * 3 / 4..].iter().sum::<f32>() / (SPECTRUM_BINS / 4) as f32;
        assert!(
            low_avg > high_avg + 10.0,
            "DC energy should be in low bins: low_avg={low_avg}, high_avg={high_avg}"
        );
    }

    #[test]
    fn fft_sine_1khz_peak_in_correct_bin() {
        let mut rb = FftRingBuffer::new();
        let mut signal = [0.0_f32; FFT_SIZE];
        for (i, s) in signal.iter_mut().enumerate() {
            *s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SAMPLE_RATE).sin();
        }
        assert!(rb.push_samples(&signal));
        let bins = rb.compute_spectrum().expect("should produce spectrum");
        // Find the peak bin
        let peak_bin = bins
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        // 1kHz should map to roughly the middle range of log-scaled bins
        // log(1000) is between log(20) and log(20000), about 57% through
        let expected_approx = (SPECTRUM_BINS as f32 * 0.57) as usize;
        let tolerance = SPECTRUM_BINS / 10; // ~10% tolerance
        assert!(
            peak_bin.abs_diff(expected_approx) < tolerance,
            "1kHz peak at bin {peak_bin}, expected near {expected_approx} (±{tolerance})"
        );
    }

    #[test]
    fn fft_silence_all_bins_near_floor() {
        let mut rb = FftRingBuffer::new();
        let silence = [0.0_f32; FFT_SIZE];
        assert!(rb.push_samples(&silence));
        let bins = rb.compute_spectrum().expect("should produce spectrum");
        for (i, &val) in bins.iter().enumerate() {
            assert!(
                val <= -99.0,
                "silence bin {i} should be at noise floor, got {val} dB"
            );
        }
    }

    #[test]
    fn ring_buffer_accumulation_and_full_detection() {
        let mut rb = FftRingBuffer::new();
        // Push less than FFT_SIZE — should not be full
        let half = [0.0_f32; FFT_SIZE / 2];
        assert!(!rb.push_samples(&half), "should not be full at half");
        assert!(
            rb.compute_spectrum().is_none(),
            "should return None when not full"
        );
        // Push the rest — should be full
        assert!(rb.push_samples(&half), "should be full now");
        assert!(
            rb.compute_spectrum().is_some(),
            "should compute after full"
        );
        // After compute, counter resets — should not be full again
        assert!(
            rb.compute_spectrum().is_none(),
            "should be None after reset"
        );
    }

    #[test]
    fn magnitude_scaling_is_db() {
        let mut rb = FftRingBuffer::new();
        // Full-scale sine at 1kHz
        let mut signal = [0.0_f32; FFT_SIZE];
        for (i, s) in signal.iter_mut().enumerate() {
            *s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SAMPLE_RATE).sin();
        }
        assert!(rb.push_samples(&signal));
        let bins = rb.compute_spectrum().expect("should produce spectrum");
        let peak_val = bins.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        // Full-scale sine should have peak near 0 dBFS (within windowing loss)
        // Hann window loses ~6dB, so expect roughly -6 to 0 dB
        assert!(
            peak_val > -12.0 && peak_val <= 0.0,
            "peak should be in dB scale near 0 dBFS, got {peak_val}"
        );
    }
}
