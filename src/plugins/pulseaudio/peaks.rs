//! Simple poll-based peak level monitor for PulseAudio sinks.
//!
//! Parses `pactl list sinks` output to extract volume percentages.
//! This is an MVP approach; true peak monitoring via PA streams
//! (pa_stream_peek with PA_STREAM_PEAK_DETECT) is a future improvement.

use std::collections::HashMap;
use std::process::Command;

use crate::plugin::api::SourceId;

/// Poll-based peak level monitor that reads sink volumes via `pactl`.
pub struct PeakMonitor {
    levels: HashMap<SourceId, f32>,
}

impl PeakMonitor {
    pub fn new() -> Self {
        Self {
            levels: HashMap::new(),
        }
    }

    /// Query `pactl list sinks`, find the sink matching `sink_name`,
    /// parse its `Volume:` line, and store the percentage as 0.0..1.0.
    ///
    /// If the sink is not found or the command fails, stores 0.0.
    pub fn update_level(&mut self, sink_name: &str, source_id: SourceId) {
        let level = read_sink_volume(sink_name).unwrap_or(0.0);
        tracing::trace!(sink_name = %sink_name, source_id = ?source_id, level = level, "peak level updated");
        let previous = self.levels.get(&source_id).copied().unwrap_or(0.0);
        if (level - previous).abs() > 0.1 {
            tracing::debug!(
                sink_name = %sink_name,
                source_id = ?source_id,
                previous = previous,
                current = level,
                delta = level - previous,
                "significant peak level change"
            );
        }
        self.levels.insert(source_id, level);
    }

    /// Return a snapshot of all tracked peak levels.
    pub fn get_levels(&self) -> HashMap<SourceId, f32> {
        self.levels.clone()
    }

    /// Clear all stored levels.
    pub fn clear(&mut self) {
        tracing::debug!(count = self.levels.len(), "clearing all peak levels");
        self.levels.clear();
    }
}

/// Run `pactl list sinks` and extract the volume percentage for `sink_name`.
///
/// Scans for a block whose `Name:` field matches, then reads the first
/// `Volume:` line in that block. Expects the format:
///   `Volume: front-left: 32768 / 50% / -18.06 dB, ...`
/// Returns the first channel's percentage as 0.0..1.0.
fn read_sink_volume(sink_name: &str) -> Option<f32> {
    let output = Command::new("pactl")
        .args(["list", "sinks"])
        .output()
        .ok()?;

    if !output.status.success() {
        tracing::warn!(sink_name = %sink_name, "pactl list sinks returned non-zero status");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_sink_volume(&stdout, sink_name);
    if result.is_none() {
        tracing::warn!(sink_name = %sink_name, "sink not found in pactl output");
    }
    result
}

/// Parse `pactl list sinks` text to find the volume for `sink_name`.
fn parse_sink_volume(pactl_output: &str, sink_name: &str) -> Option<f32> {
    let mut in_target_sink = false;

    for line in pactl_output.lines() {
        let trimmed = line.trim();

        // Detect sink block boundaries (non-indented "Sink #N" lines).
        if trimmed.starts_with("Sink #") {
            in_target_sink = false;
            continue;
        }

        // Match the Name: field inside a sink block.
        if let Some(name) = trimmed.strip_prefix("Name:") {
            if name.trim() == sink_name {
                in_target_sink = true;
            }
            continue;
        }

        // Once inside the target sink, find the Volume: line.
        if in_target_sink {
            if let Some(rest) = trimmed.strip_prefix("Volume:") {
                return parse_volume_percentage(rest);
            }
        }
    }

    None
}

/// Extract the first percentage value from a PA Volume line fragment.
///
/// Input example: ` front-left: 32768 / 50% / -18.06 dB, front-right: ...`
/// Returns 0.5 for 50%.
fn parse_volume_percentage(volume_line: &str) -> Option<f32> {
    for segment in volume_line.split('/') {
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

    const PACTL_OUTPUT: &str = "\
Sink #0
	Name: alsa_output.pci-0000_00_1f.3.analog-stereo
	Description: Built-in Audio Analog Stereo
	Volume: front-left: 32768 / 50% / -18.06 dB,   front-right: 32768 / 50% / -18.06 dB
	Mute: no

Sink #1
	Name: osg_Music_Apps
	Description: OSG Music Channel
	Volume: front-left: 48000 / 73% / -8.06 dB,   front-right: 48000 / 73% / -8.06 dB
	Mute: no

Sink #2
	Name: osg_Comms_Apps
	Description: OSG Comms Channel
	Volume: front-left: 65536 / 100% / 0.00 dB,   front-right: 65536 / 100% / 0.00 dB
	Mute: no
";

    #[test]
    fn parses_known_sink() {
        let level = parse_sink_volume(PACTL_OUTPUT, "osg_Music_Apps");
        assert!((level.unwrap() - 0.73).abs() < 0.01);
    }

    #[test]
    fn parses_full_volume() {
        let level = parse_sink_volume(PACTL_OUTPUT, "osg_Comms_Apps");
        assert!((level.unwrap() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn returns_none_for_missing_sink() {
        let level = parse_sink_volume(PACTL_OUTPUT, "nonexistent_sink");
        assert!(level.is_none());
    }

    #[test]
    fn update_stores_zero_for_missing_sink() {
        let mut monitor = PeakMonitor::new();
        // read_sink_volume will fail (no pactl in test env), so falls back to 0.0
        monitor.update_level("fake_sink", SourceId::Channel(1));
        assert!((monitor.get_levels()[&SourceId::Channel(1)] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn clear_empties_levels() {
        let mut monitor = PeakMonitor::new();
        monitor.levels.insert(SourceId::Channel(1), 0.5);
        monitor.clear();
        assert!(monitor.get_levels().is_empty());
    }

    #[test]
    fn parse_volume_percentage_extracts_first() {
        let pct = parse_volume_percentage(" front-left: 32768 / 50% / -18.06 dB");
        assert!((pct.unwrap() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_volume_percentage_clamps_overflow() {
        let pct = parse_volume_percentage(" front-left: 99999 / 153% / 5.00 dB");
        assert!((pct.unwrap() - 1.0).abs() < f32::EPSILON);
    }
}
