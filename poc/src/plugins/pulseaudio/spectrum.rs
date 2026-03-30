//! FFT spectrum analysis for PA monitor sources.
//!
//! Captures PCM from a null-sink's monitor source via `parecord`, runs realfft
//! on 1024-sample windows with Hann windowing, and returns frequency bins as
//! `(hz, db)` pairs suitable for the EQ canvas spectrum overlay.
//!
//! ## Current implementation
//!
//! Uses `parecord` subprocess to capture a short burst of samples from a sink's
//! `.monitor` source. This is a pragmatic first pass — the same approach used
//! by `peaks.rs` for volume polling. A future revision will replace this with
//! native PA stream capture for lower latency and higher throughput.

use realfft::RealFftPlanner;
use tracing::{debug, instrument, trace, warn};

/// Number of samples per FFT window.
pub const FFT_SIZE: usize = 1024;

/// Sample rate (PA default).
const SAMPLE_RATE: f32 = 48_000.0;

/// Convert raw PCM f32 samples to frequency bins.
///
/// Returns a `Vec<(freq_hz, amplitude_db)>` suitable for the spectrum overlay.
/// Applies a Hann window before FFT to reduce spectral leakage.
#[instrument(skip(samples), fields(sample_count = samples.len()))]
pub fn samples_to_spectrum(samples: &[f32]) -> Vec<(f32, f32)> {
    trace!("computing FFT spectrum from {} samples", samples.len());

    if samples.len() < FFT_SIZE {
        debug!(
            need = FFT_SIZE,
            have = samples.len(),
            "not enough samples for FFT"
        );
        return Vec::new();
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let mut input: Vec<f32> = samples[..FFT_SIZE].to_vec();

    // Apply Hann window to reduce spectral leakage
    for (i, sample) in input.iter_mut().enumerate() {
        let w =
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos());
        *sample *= w;
    }

    let mut output = fft.make_output_vec();

    if let Err(e) = fft.process(&mut input, &mut output) {
        warn!(error = %e, "FFT processing failed");
        return Vec::new();
    }

    let freq_resolution = SAMPLE_RATE / FFT_SIZE as f32;

    let bins: Vec<(f32, f32)> = output
        .iter()
        .enumerate()
        .skip(1) // skip DC
        .take(output.len() - 1) // positive frequencies only
        .map(|(i, complex)| {
            let freq = i as f32 * freq_resolution;
            let magnitude = complex.norm();
            let db = if magnitude > 1e-10 {
                20.0 * magnitude.log10()
            } else {
                -100.0
            };
            (freq, db)
        })
        .filter(|(freq, _)| *freq >= 20.0 && *freq <= 20_000.0)
        .collect();

    debug!(bin_count = bins.len(), "FFT spectrum computed");
    bins
}

/// Capture a short burst of PCM samples from a PA sink's monitor source.
///
/// Uses `parecord` to record `FFT_SIZE` samples from `{sink_name}.monitor`.
/// Returns raw f32 samples on success, empty vec on failure.
#[instrument]
pub fn capture_monitor_samples(sink_name: &str) -> Vec<f32> {
    let monitor_source = format!("{sink_name}.monitor");
    let duration_ms = ((FFT_SIZE as f32 / SAMPLE_RATE) * 1000.0).ceil() as u64 + 5; // +5ms margin

    trace!(
        monitor = %monitor_source,
        duration_ms,
        fft_size = FFT_SIZE,
        "capturing PCM from PA monitor source"
    );

    // Use parecord to capture raw f32le samples
    let result = std::process::Command::new("parecord")
        .args([
            "--device",
            &monitor_source,
            "--format=float32le",
            "--channels=1",
            "--rate=48000",
            "--raw",
            "--process-time-msec",
            &duration_ms.to_string(),
            "/dev/stdout",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match result {
        Ok(output) => {
            if output.status.success() && output.stdout.len() >= 4 {
                let samples: Vec<f32> = output
                    .stdout
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                debug!(
                    captured_samples = samples.len(),
                    "monitor capture successful"
                );
                samples
            } else {
                debug!(
                    status = %output.status,
                    bytes = output.stdout.len(),
                    "parecord returned insufficient data"
                );
                Vec::new()
            }
        }
        Err(e) => {
            debug!(error = %e, "parecord failed — spectrum unavailable for this sink");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_samples_to_spectrum_sine_wave() {
        // Generate a 1kHz sine wave
        let freq = 1000.0;
        let samples: Vec<f32> = (0..FFT_SIZE)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / SAMPLE_RATE).sin())
            .collect();

        let bins = samples_to_spectrum(&samples);
        assert!(!bins.is_empty(), "should produce frequency bins");

        // Find peak bin — should be near 1000 Hz
        let peak = bins
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        assert!(
            (peak.0 - 1000.0).abs() < 100.0,
            "peak frequency {} should be near 1000 Hz",
            peak.0
        );
    }

    #[test]
    fn test_samples_to_spectrum_too_few_samples() {
        let samples = vec![0.0; 100];
        let bins = samples_to_spectrum(&samples);
        assert!(
            bins.is_empty(),
            "should return empty for insufficient samples"
        );
    }

    #[test]
    fn test_samples_to_spectrum_silence() {
        let samples = vec![0.0; FFT_SIZE];
        let bins = samples_to_spectrum(&samples);
        // All bins should be very low dB
        for (_, db) in &bins {
            assert!(
                *db < -60.0,
                "silence should have low amplitude, got {} dB",
                db
            );
        }
    }

    #[test]
    fn test_samples_to_spectrum_multiple_frequencies() {
        // Generate a signal with 440Hz and 2000Hz components
        let samples: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE;
                0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                    + 0.3 * (2.0 * std::f32::consts::PI * 2000.0 * t).sin()
            })
            .collect();

        let bins = samples_to_spectrum(&samples);
        assert!(bins.len() > 10, "should have many frequency bins");

        // Should have energy around both 440Hz and 2000Hz
        let has_440 = bins
            .iter()
            .any(|(f, db)| (*f - 440.0).abs() < 100.0 && *db > -40.0);
        let has_2k = bins
            .iter()
            .any(|(f, db)| (*f - 2000.0).abs() < 100.0 && *db > -40.0);
        assert!(has_440, "should detect 440Hz component");
        assert!(has_2k, "should detect 2000Hz component");
    }

    #[test]
    fn test_hann_window_reduces_leakage() {
        // A sine wave should produce a sharp peak with Hann windowing
        let freq = 1000.0;
        let samples: Vec<f32> = (0..FFT_SIZE)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / SAMPLE_RATE).sin())
            .collect();

        let bins = samples_to_spectrum(&samples);
        let peak = bins
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        // Most energy should be concentrated near the peak
        let high_energy_bins: Vec<_> = bins.iter().filter(|(_, db)| *db > peak.1 - 20.0).collect();
        assert!(
            high_energy_bins.len() < 10,
            "Hann window should concentrate energy: {} bins within 20dB of peak",
            high_energy_bins.len()
        );
    }
}
