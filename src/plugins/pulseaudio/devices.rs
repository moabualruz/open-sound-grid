//! PulseAudio device enumeration via `pactl`.
//!
//! Parses `pactl list sinks` and `pactl list sources` to discover
//! hardware audio devices, filtering out virtual sinks/sources
//! created by OpenSoundGrid or other software.

use std::process::Command;

use crate::plugin::api::{HardwareInput, HardwareOutput};

/// Filters applied to sink names to exclude virtual/software sinks.
const SINK_EXCLUDE_PATTERNS: &[&str] = &["_ch", "_mix", "_Apps", "_OBS"];

/// Filters applied to source names to exclude monitor sources.
const SOURCE_EXCLUDE_PATTERNS: &[&str] = &[".monitor"];

/// Parsed fields from a single `pactl list sinks/sources` section.
struct PactlDevice {
    index: u32,
    name: String,
    description: String,
    #[allow(dead_code)]
    state: String,
}

pub struct DeviceEnumerator;

impl DeviceEnumerator {
    /// List hardware audio outputs by parsing `pactl list sinks`.
    ///
    /// Filters out sinks whose Name contains any of the patterns in
    /// [`SINK_EXCLUDE_PATTERNS`] (virtual sinks created by OSG, OBS, etc.).
    pub fn list_outputs() -> Vec<HardwareOutput> {
        let output = match Command::new("pactl")
            .args(["list", "sinks"])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("[DeviceEnumerator] pactl list sinks failed: {e}");
                return Vec::new();
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices = parse_sections(&stdout, "Sink");

        devices
            .into_iter()
            .filter(|d| {
                !SINK_EXCLUDE_PATTERNS
                    .iter()
                    .any(|pat| d.name.contains(pat))
            })
            .map(|d| HardwareOutput {
                id: d.index,
                name: d.description.clone(),
                description: d.description,
                device_id: d.name,
            })
            .collect()
    }

    /// List hardware audio inputs by parsing `pactl list sources`.
    ///
    /// Filters out sources whose Name contains `.monitor` (PA creates
    /// a monitor source for every sink automatically).
    pub fn list_inputs() -> Vec<HardwareInput> {
        let output = match Command::new("pactl")
            .args(["list", "sources"])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("[DeviceEnumerator] pactl list sources failed: {e}");
                return Vec::new();
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices = parse_sections(&stdout, "Source");

        devices
            .into_iter()
            .filter(|d| {
                !SOURCE_EXCLUDE_PATTERNS
                    .iter()
                    .any(|pat| d.name.contains(pat))
            })
            .map(|d| HardwareInput {
                id: d.index,
                name: d.description.clone(),
                description: d.description,
            })
            .collect()
    }
}

/// Parse `pactl list sinks` or `pactl list sources` output into device records.
///
/// Sections start with `{kind} #{index}` (e.g. `Sink #42`) and fields are
/// tab-indented as `\tFieldName: value`.
fn parse_sections(output: &str, kind: &str) -> Vec<PactlDevice> {
    let section_prefix = format!("{kind} #");
    let mut devices = Vec::new();
    let mut current: Option<PactlDevice> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix(&section_prefix) {
            // Flush previous section.
            if let Some(dev) = current.take() {
                devices.push(dev);
            }

            let index = rest
                .trim()
                .parse::<u32>()
                .unwrap_or(0);

            current = Some(PactlDevice {
                index,
                name: String::new(),
                description: String::new(),
                state: String::new(),
            });
        } else if let Some(ref mut dev) = current {
            // Fields are tab-indented: `\tName: value`
            let trimmed = line.trim_start_matches('\t');

            // Only parse top-level fields (single tab indent).
            // Nested properties (double tab) are skipped.
            if line.starts_with('\t') && !line.starts_with("\t\t") {
                if let Some(value) = trimmed.strip_prefix("Name: ") {
                    dev.name = value.trim().to_string();
                } else if let Some(value) = trimmed.strip_prefix("Description: ") {
                    dev.description = value.trim().to_string();
                } else if let Some(value) = trimmed.strip_prefix("State: ") {
                    dev.state = value.trim().to_string();
                }
            }
        }
    }

    // Flush last section.
    if let Some(dev) = current.take() {
        devices.push(dev);
    }

    devices
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACTL_SINKS: &str = "\
Sink #0
\tState: RUNNING
\tName: alsa_output.pci-0000_00_1f.3.analog-stereo
\tDescription: Built-in Audio Analog Stereo
\tDriver: module-alsa-card.c

Sink #1
\tState: IDLE
\tName: osg_Music_ch
\tDescription: OSG Music Channel
\tDriver: module-null-sink.c

Sink #2
\tState: IDLE
\tName: osg_Main_mix
\tDescription: OSG Main Mix
\tDriver: module-null-sink.c

Sink #3
\tState: IDLE
\tName: osg_Discord_Apps
\tDescription: OSG Discord Apps
\tDriver: module-null-sink.c

Sink #4
\tState: IDLE
\tName: obs_sink_OBS
\tDescription: OBS Virtual Sink
\tDriver: module-null-sink.c

Sink #5
\tState: SUSPENDED
\tName: alsa_output.usb-SteelSeries_Arctis_7-00.analog-stereo
\tDescription: SteelSeries Arctis 7 Analog Stereo
\tDriver: module-alsa-card.c
";

    const PACTL_SOURCES: &str = "\
Source #0
\tState: RUNNING
\tName: alsa_input.pci-0000_00_1f.3.analog-stereo
\tDescription: Built-in Audio Analog Stereo
\tDriver: module-alsa-card.c

Source #1
\tState: IDLE
\tName: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
\tDescription: Monitor of Built-in Audio Analog Stereo
\tDriver: module-alsa-card.c

Source #2
\tState: SUSPENDED
\tName: alsa_input.usb-Blue_Yeti-00.analog-stereo
\tDescription: Blue Yeti Analog Stereo
\tDriver: module-alsa-card.c

Source #3
\tState: IDLE
\tName: osg_Music_ch.monitor
\tDescription: Monitor of OSG Music Channel
\tDriver: module-null-sink.c
";

    #[test]
    fn parse_sinks_filters_virtual() {
        let devices = parse_sections(PACTL_SINKS, "Sink");
        assert_eq!(devices.len(), 6);

        // Apply the same filter as list_outputs.
        let filtered: Vec<_> = devices
            .into_iter()
            .filter(|d| {
                !SINK_EXCLUDE_PATTERNS
                    .iter()
                    .any(|pat| d.name.contains(pat))
            })
            .collect();

        // Only the two real ALSA sinks should remain.
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].index, 0);
        assert_eq!(filtered[0].name, "alsa_output.pci-0000_00_1f.3.analog-stereo");
        assert_eq!(filtered[0].description, "Built-in Audio Analog Stereo");

        assert_eq!(filtered[1].index, 5);
        assert_eq!(filtered[1].name, "alsa_output.usb-SteelSeries_Arctis_7-00.analog-stereo");
    }

    #[test]
    fn parse_sources_filters_monitors() {
        let devices = parse_sections(PACTL_SOURCES, "Source");
        assert_eq!(devices.len(), 4);

        let filtered: Vec<_> = devices
            .into_iter()
            .filter(|d| {
                !SOURCE_EXCLUDE_PATTERNS
                    .iter()
                    .any(|pat| d.name.contains(pat))
            })
            .collect();

        // Only the two real input devices should remain.
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].index, 0);
        assert_eq!(filtered[0].name, "alsa_input.pci-0000_00_1f.3.analog-stereo");

        assert_eq!(filtered[1].index, 2);
        assert_eq!(filtered[1].name, "alsa_input.usb-Blue_Yeti-00.analog-stereo");
    }

    #[test]
    fn empty_output_produces_no_devices() {
        let devices = parse_sections("", "Sink");
        assert!(devices.is_empty());
    }
}
