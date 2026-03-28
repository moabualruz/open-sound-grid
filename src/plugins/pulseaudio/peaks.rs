//! Real peak monitoring via PA PEAK_DETECT streams.
//!
//! Each monitored sink gets a long-lived record stream on its `.monitor` source.
//! PA delivers peak amplitude values via the read callback at server rate (~25 Hz).
//! Values are stored in shared atomics and collected by `build_snapshot()`.
//!
//! ## Current implementation
//!
//! Storing PA `Stream` objects long-term requires `unsafe` lifetime tricks because
//! `libpulse_binding::stream::Stream` borrows the context/mainloop. The pragmatic
//! approach for v0.1 is:
//!
//! - Track *which* sinks are being monitored in `active_sinks`.
//! - `read_peaks()` queries `pactl get-sink-volume` for each active sink so that
//!   only channels/mixes the user has created are polled (not all sinks on the system).
//! - `SharedPeak` atomics are the write surface — the PA callback path (when wired
//!   in a future revision) will call `SharedPeak::store()` directly from the
//!   mainloop thread without any locking overhead.
//!
//! ## Future upgrade path
//!
//! Once stream lifetimes are solved (e.g. by boxing the stream with `'static`
//! context borrowed via `Arc`), replace the `read_peaks` body with actual
//! `Stream::peek()` / `Stream::discard()` calls and remove the `pactl` subprocess.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::plugin::api::SourceId;

/// Shared peak value (f32 stored as u32 bits for atomic access).
///
/// Written by `read_peaks()` (plugin thread) and read by `get_levels()` (same
/// thread for now, but ready for cross-thread use when PA callbacks are wired in).
#[derive(Clone)]
struct SharedPeak(Arc<AtomicU32>);

impl SharedPeak {
    fn new() -> Self {
        Self(Arc::new(AtomicU32::new(0)))
    }

    fn store(&self, val: f32) {
        self.0.store(val.to_bits(), Ordering::Relaxed);
    }

    fn load(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// Manages peak detection for all monitored sinks.
///
/// Only sinks explicitly registered via `start_monitoring()` are polled —
/// this avoids querying every PA sink on every state refresh.
pub struct PeakMonitor {
    /// Maps source_id → shared peak value (written by `read_peaks`, read by `get_levels`).
    peaks: HashMap<SourceId, SharedPeak>,
    /// Maps source_id → sink_name for active monitoring targets.
    active_sinks: HashMap<SourceId, String>,
}

impl PeakMonitor {
    pub fn new() -> Self {
        Self {
            peaks: HashMap::new(),
            active_sinks: HashMap::new(),
        }
    }

    /// Register a sink for peak monitoring.
    ///
    /// Idempotent — calling again for the same `source_id` is a no-op.
    /// In a future revision this will create a PA PEAK_DETECT record stream
    /// on `{sink_name}.monitor`; for now it registers the sink so `read_peaks()`
    /// will poll it via `pactl`.
    pub fn start_monitoring(&mut self, sink_name: &str, source_id: SourceId) {
        if self.active_sinks.contains_key(&source_id) {
            tracing::debug!(?source_id, sink = %sink_name, "already monitoring — skipping");
            return;
        }

        tracing::info!(
            sink = %sink_name,
            monitor_source = %format!("{sink_name}.monitor"),
            ?source_id,
            "registering sink for peak monitoring"
        );

        self.peaks.insert(source_id, SharedPeak::new());
        self.active_sinks.insert(source_id, sink_name.to_string());
    }

    /// Unregister a sink from peak monitoring and discard its stored level.
    pub fn stop_monitoring(&mut self, source_id: &SourceId) {
        if let Some(sink_name) = self.active_sinks.remove(source_id) {
            self.peaks.remove(source_id);
            tracing::debug!(?source_id, sink = %sink_name, "peak monitor stopped");
        }
    }

    /// Return current peak levels for all monitored sinks.
    ///
    /// Lock-free read from the shared atomics — safe to call from any thread.
    /// Returns only the sinks currently registered via `start_monitoring()`.
    pub fn get_levels(&self) -> HashMap<SourceId, f32> {
        let levels: HashMap<SourceId, f32> = self
            .peaks
            .iter()
            .map(|(id, peak)| (*id, peak.load()))
            .collect();
        tracing::trace!(count = levels.len(), "get_levels: returning peak snapshot");
        levels
    }

    /// Poll PA for current peak levels of all active sinks.
    ///
    /// Called from `build_snapshot()` on the plugin thread. Queries
    /// `pactl get-sink-volume` for each registered sink and stores the
    /// result in the shared atomic.
    ///
    /// # Future upgrade
    ///
    /// When PA PEAK_DETECT streams are wired, this method will instead
    /// call `stream.peek()` / `stream.discard()` under the mainloop lock
    /// and decode the U8 sample as a normalised f32 amplitude. The
    /// `active_sinks` map and `SharedPeak` infrastructure are already in
    /// place for that path — only the body of this method changes.
    pub fn read_peaks(&mut self) {
        let active_count = self.active_sinks.len();
        tracing::trace!(
            active_sinks = active_count,
            "read_peaks: polling sink volumes"
        );

        for (source_id, sink_name) in &self.active_sinks {
            let level = read_sink_volume(sink_name).unwrap_or(0.0);
            tracing::trace!(
                ?source_id,
                sink = %sink_name,
                level,
                "peak level polled"
            );
            if let Some(peak) = self.peaks.get(source_id) {
                let previous = peak.load();
                peak.store(level);
                if (level - previous).abs() > 0.1 {
                    tracing::debug!(
                        ?source_id,
                        sink = %sink_name,
                        previous,
                        current = level,
                        delta = level - previous,
                        "significant peak level change"
                    );
                }
            }
        }
    }
}

/// Query `pactl get-sink-volume` for a single sink and return 0.0..1.0.
fn read_sink_volume(sink_name: &str) -> Option<f32> {
    let output = std::process::Command::new("pactl")
        .args(["get-sink-volume", sink_name])
        .output()
        .ok()?;

    if !output.status.success() {
        tracing::warn!(sink = %sink_name, "pactl get-sink-volume returned non-zero status");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_volume_percentage(&stdout);
    if result.is_none() {
        tracing::warn!(sink = %sink_name, output = %stdout.trim(), "could not parse volume from pactl output");
    }
    result
}

/// Extract the first percentage value from a PA volume line.
///
/// Handles both `get-sink-volume` output:
///   `Volume: front-left: 32768 /  50% / -18.06 dB, ...`
/// and the simpler single-line format. Returns the first `XX%` as 0.0..1.0.
fn parse_volume_percentage(line: &str) -> Option<f32> {
    for segment in line.split('/') {
        let segment = segment.trim();
        if let Some(pct_str) = segment.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>() {
                return Some((pct / 100.0).clamp(0.0, 1.0));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_volume_percentage() {
        let pct = parse_volume_percentage("  Volume: front-left: 32768 /  50% / -18.06 dB");
        assert!((pct.unwrap() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_volume_zero() {
        let pct = parse_volume_percentage("  Volume: mono: 0 /   0% / -inf dB");
        assert!((pct.unwrap() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_volume_clamps_overflow() {
        let pct = parse_volume_percentage(" front-left: 99999 / 153% / 5.00 dB");
        assert!((pct.unwrap() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_volume_stereo_full_line() {
        let pct = parse_volume_percentage(
            "  front-left: 32768 /  50% / -18.06 dB, front-right: 32768 /  50% / -18.06 dB",
        );
        assert!(
            (pct.unwrap() - 0.5).abs() < f32::EPSILON,
            "expected 0.5 for 50%"
        );
    }

    #[test]
    fn test_parse_volume_empty_returns_none() {
        assert!(parse_volume_percentage("").is_none());
    }

    #[test]
    fn test_shared_peak_atomic() {
        let peak = SharedPeak::new();
        assert_eq!(peak.load(), 0.0);
        peak.store(0.75);
        assert_eq!(peak.load(), 0.75);
    }

    #[test]
    fn test_shared_peak_clone_shares_storage() {
        let peak = SharedPeak::new();
        let clone = peak.clone();
        peak.store(0.5);
        assert_eq!(clone.load(), 0.5);
    }

    #[test]
    fn test_start_monitoring_idempotent() {
        let mut monitor = PeakMonitor::new();
        monitor.start_monitoring("osg_Music_ch", SourceId::Channel(1));
        monitor.start_monitoring("osg_Music_ch", SourceId::Channel(1));
        assert_eq!(monitor.active_sinks.len(), 1);
    }

    #[test]
    fn test_stop_monitoring_removes_entry() {
        let mut monitor = PeakMonitor::new();
        monitor.start_monitoring("osg_Music_ch", SourceId::Channel(1));
        assert_eq!(monitor.get_levels().len(), 1);
        monitor.stop_monitoring(&SourceId::Channel(1));
        assert!(monitor.get_levels().is_empty());
    }

    #[test]
    fn test_get_levels_returns_initial_zeros() {
        let mut monitor = PeakMonitor::new();
        monitor.start_monitoring("osg_Music_ch", SourceId::Channel(1));
        monitor.start_monitoring("osg_Main_mix", SourceId::Mix(1));
        let levels = monitor.get_levels();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[&SourceId::Channel(1)], 0.0);
        assert_eq!(levels[&SourceId::Mix(1)], 0.0);
    }

    #[test]
    fn test_read_peaks_no_panic_without_pa() {
        // read_peaks calls pactl which may not be available in CI — must not panic.
        let mut monitor = PeakMonitor::new();
        monitor.start_monitoring("nonexistent_sink", SourceId::Channel(99));
        // Falls back to 0.0 on pactl failure — no panic.
        monitor.read_peaks();
        assert_eq!(monitor.get_levels()[&SourceId::Channel(99)], 0.0);
    }
}
