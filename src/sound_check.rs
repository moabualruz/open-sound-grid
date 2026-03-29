//! Sound Check — record, playback, and waveform visualization.
//!
//! Records mic input via PA stream capture for a configurable duration,
//! stores the PCM buffer, and plays it back in a loop through the
//! channel's effects chain. A waveform visualization is derived from
//! the stored samples.
//!
//! ## Current implementation
//!
//! The `SoundCheckBuffer` manages the state machine and PCM storage.
//! PA stream capture and playback integration is handled by the plugin
//! layer — the buffer just accumulates samples and provides waveform peaks.

use tracing::{debug, info, instrument, trace};

/// Default recording duration in seconds.
const DEFAULT_RECORD_SECONDS: f32 = 5.0;

/// Sample rate (PA default).
const SAMPLE_RATE: u32 = 48_000;

/// State machine for the sound check workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCheckState {
    /// Ready to record — no recording in progress.
    Idle,
    /// Currently recording mic input.
    Recording,
    /// Recording complete — buffer ready for playback.
    Ready,
    /// Playing back the recorded buffer in a loop.
    Playing,
}

impl Default for SoundCheckState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Holds the recorded PCM samples and playback state.
#[derive(Debug, Clone)]
pub struct SoundCheckBuffer {
    /// Raw f32 PCM samples (mono, 48kHz).
    pub samples: Vec<f32>,
    /// Current playback position (sample index).
    pub playback_pos: usize,
    /// Duration of the recording in seconds.
    pub duration_secs: f32,
    /// Current state.
    pub state: SoundCheckState,
}

impl Default for SoundCheckBuffer {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            playback_pos: 0,
            duration_secs: DEFAULT_RECORD_SECONDS,
            state: SoundCheckState::Idle,
        }
    }
}

impl SoundCheckBuffer {
    /// Create a new empty buffer with the given recording duration.
    #[instrument]
    pub fn new(duration_secs: f32) -> Self {
        info!(duration_secs, "creating sound check buffer");
        Self {
            duration_secs,
            ..Default::default()
        }
    }

    /// Start recording — clears any existing buffer.
    #[instrument(skip(self))]
    pub fn start_recording(&mut self) {
        info!("sound check: starting recording");
        self.samples.clear();
        self.playback_pos = 0;
        self.state = SoundCheckState::Recording;
    }

    /// Append samples to the recording buffer.
    #[instrument(skip(self, samples), fields(new_samples = samples.len()))]
    pub fn append_samples(&mut self, samples: &[f32]) {
        trace!(
            current_len = self.samples.len(),
            new_len = samples.len(),
            "appending samples to buffer"
        );
        let max_samples = (self.duration_secs * SAMPLE_RATE as f32) as usize;
        let remaining = max_samples.saturating_sub(self.samples.len());
        let to_take = samples.len().min(remaining);
        self.samples.extend_from_slice(&samples[..to_take]);

        if self.samples.len() >= max_samples {
            debug!(
                total_samples = self.samples.len(),
                "recording buffer full — stopping"
            );
            self.state = SoundCheckState::Ready;
        }
    }

    /// Check if the recording buffer is full.
    pub fn is_buffer_full(&self) -> bool {
        let max = (self.duration_secs * SAMPLE_RATE as f32) as usize;
        self.samples.len() >= max
    }

    /// Stop recording early.
    #[instrument(skip(self))]
    pub fn stop_recording(&mut self) {
        info!(
            samples = self.samples.len(),
            "sound check: recording stopped"
        );
        if !self.samples.is_empty() {
            self.state = SoundCheckState::Ready;
        } else {
            self.state = SoundCheckState::Idle;
        }
    }

    /// Start looped playback.
    #[instrument(skip(self))]
    pub fn start_playback(&mut self) {
        info!("sound check: starting playback loop");
        self.playback_pos = 0;
        self.state = SoundCheckState::Playing;
    }

    /// Stop playback.
    #[instrument(skip(self))]
    pub fn stop_playback(&mut self) {
        info!("sound check: stopping playback");
        self.state = SoundCheckState::Ready;
    }

    /// Generate waveform visualization data.
    ///
    /// Returns `num_points` peak values suitable for rendering a waveform.
    /// Each value is the maximum absolute amplitude in its time window.
    #[instrument(skip(self), fields(total_samples = self.samples.len()))]
    pub fn waveform_peaks(&self, num_points: usize) -> Vec<f32> {
        trace!(num_points, "generating waveform peaks");
        if self.samples.is_empty() || num_points == 0 {
            return vec![0.0; num_points];
        }

        let chunk_size = (self.samples.len() / num_points).max(1);
        self.samples
            .chunks(chunk_size)
            .take(num_points)
            .map(|chunk| chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max))
            .collect()
    }

    /// Recording progress as a fraction (0.0 to 1.0).
    pub fn recording_progress(&self) -> f32 {
        let max = (self.duration_secs * SAMPLE_RATE as f32) as usize;
        if max == 0 {
            return 0.0;
        }
        (self.samples.len() as f32 / max as f32).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_idle() {
        let buf = SoundCheckBuffer::new(5.0);
        assert_eq!(buf.state, SoundCheckState::Idle);
        assert!(buf.samples.is_empty());
    }

    #[test]
    fn test_start_recording_clears_buffer() {
        let mut buf = SoundCheckBuffer::new(5.0);
        buf.samples = vec![1.0; 100];
        buf.start_recording();
        assert!(buf.samples.is_empty());
        assert_eq!(buf.state, SoundCheckState::Recording);
    }

    #[test]
    fn test_append_samples_fills_buffer() {
        let mut buf = SoundCheckBuffer::new(0.1); // 0.1s = 4800 samples at 48kHz
        buf.start_recording();
        buf.append_samples(&vec![0.5; 4800]);
        assert_eq!(buf.state, SoundCheckState::Ready);
        assert!(buf.is_buffer_full());
    }

    #[test]
    fn test_append_samples_does_not_overflow() {
        let mut buf = SoundCheckBuffer::new(0.1);
        buf.start_recording();
        buf.append_samples(&vec![0.5; 10000]); // more than 4800
        assert!(buf.samples.len() <= 4800);
    }

    #[test]
    fn test_stop_recording_with_data_goes_ready() {
        let mut buf = SoundCheckBuffer::new(5.0);
        buf.start_recording();
        buf.append_samples(&vec![0.5; 100]);
        buf.stop_recording();
        assert_eq!(buf.state, SoundCheckState::Ready);
    }

    #[test]
    fn test_stop_recording_empty_goes_idle() {
        let mut buf = SoundCheckBuffer::new(5.0);
        buf.start_recording();
        buf.stop_recording();
        assert_eq!(buf.state, SoundCheckState::Idle);
    }

    #[test]
    fn test_playback_lifecycle() {
        let mut buf = SoundCheckBuffer::new(5.0);
        buf.start_recording();
        buf.append_samples(&vec![0.5; 100]);
        buf.stop_recording();
        buf.start_playback();
        assert_eq!(buf.state, SoundCheckState::Playing);
        buf.stop_playback();
        assert_eq!(buf.state, SoundCheckState::Ready);
    }

    #[test]
    fn test_waveform_peaks_empty() {
        let buf = SoundCheckBuffer::default();
        let peaks = buf.waveform_peaks(100);
        assert_eq!(peaks.len(), 100);
        assert!(peaks.iter().all(|&p| p == 0.0));
    }

    #[test]
    fn test_waveform_peaks_with_data() {
        let mut buf = SoundCheckBuffer::new(5.0);
        buf.samples = vec![0.0; 1000];
        buf.samples[500] = 0.8; // spike in the middle
        let peaks = buf.waveform_peaks(10);
        assert_eq!(peaks.len(), 10);
        assert!(peaks.iter().any(|&p| p > 0.5));
    }

    #[test]
    fn test_recording_progress() {
        let mut buf = SoundCheckBuffer::new(1.0); // 1s = 48000 samples
        assert_eq!(buf.recording_progress(), 0.0);
        buf.start_recording();
        buf.append_samples(&vec![0.0; 24000]);
        let progress = buf.recording_progress();
        assert!(
            (progress - 0.5).abs() < 0.01,
            "expected ~0.5, got {progress}"
        );
    }
}
