use std::process::Command;

use crate::error::{OsgError, Result};
use crate::plugin::api::AudioApplication;

/// Detects running audio applications via PulseAudio sink-inputs.
pub struct AppDetector;

impl AppDetector {
    pub fn new() -> Self {
        Self
    }

    /// List all running audio applications visible to PulseAudio.
    ///
    /// Shells out to `pactl list sink-inputs` and parses the output.
    /// Filters out sink-inputs with no `application.name` and those
    /// whose `media.name` contains "loopback" (our loopback modules).
    pub fn list_applications(&self) -> Result<Vec<AudioApplication>> {
        tracing::debug!("listing audio applications via pactl");

        let output = Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()
            .map_err(|e| {
                tracing::error!(err = %e, "pactl list sink-inputs failed to execute");
                OsgError::PulseAudio(format!("failed to run pactl: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(status = %output.status, stderr = %stderr, "pactl list sink-inputs returned error");
            return Err(OsgError::PulseAudio(format!(
                "pactl exited with {}: {stderr}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let apps = parse_sink_inputs(&stdout);
        tracing::debug!(count = apps.len(), "audio applications detected");
        for app in &apps {
            tracing::debug!(
                app_name = %app.name,
                binary = %app.binary,
                stream_index = app.stream_index,
                "detected audio application"
            );
        }
        Ok(apps)
    }
}

/// A single sink-input section parsed from `pactl list sink-inputs`.
struct SinkInputEntry {
    index: u32,
    app_name: Option<String>,
    app_binary: Option<String>,
    icon_name: Option<String>,
    media_name: Option<String>,
}

impl SinkInputEntry {
    fn new(index: u32) -> Self {
        Self {
            index,
            app_name: None,
            app_binary: None,
            icon_name: None,
            media_name: None,
        }
    }

    /// Convert to `AudioApplication` if this entry passes filters.
    fn into_application(self) -> Option<AudioApplication> {
        // Filter: must have an application name.
        let name = match self.app_name {
            Some(n) => n,
            None => {
                tracing::debug!(index = self.index, reason = "no application.name property", "filtering out sink-input");
                return None;
            }
        };

        // Filter: skip loopback streams (our loopback modules).
        if let Some(ref media) = self.media_name {
            if media.to_lowercase().contains("loopback") {
                tracing::debug!(index = self.index, app_name = %name, media_name = %media, reason = "loopback media stream", "filtering out sink-input");
                return None;
            }
        }

        Some(AudioApplication {
            id: self.index,
            name,
            binary: self.app_binary.unwrap_or_default(),
            icon_name: self.icon_name,
            stream_index: self.index,
            channel: None,
        })
    }
}

/// Parse the full output of `pactl list sink-inputs` into applications.
fn parse_sink_inputs(output: &str) -> Vec<AudioApplication> {
    let mut apps = Vec::new();
    let mut current: Option<SinkInputEntry> = None;
    let mut in_properties = false;

    for line in output.lines() {
        // New sink-input section: "Sink Input #NNN"
        if let Some(index) = parse_sink_input_header(line) {
            if let Some(entry) = current.take() {
                if let Some(app) = entry.into_application() {
                    apps.push(app);
                }
            }
            tracing::trace!(sink_input_index = index, "parsing sink-input entry");
            current = Some(SinkInputEntry::new(index));
            in_properties = false;
            continue;
        }

        let Some(ref mut entry) = current else {
            continue;
        };

        let trimmed = line.trim();

        // Detect the Properties section.
        if trimmed == "Properties:" {
            in_properties = true;
            continue;
        }

        // A non-indented line (or a top-level field) exits the properties block.
        if in_properties && !line.starts_with('\t') && !line.starts_with("  ") {
            in_properties = false;
        }

        if !in_properties {
            continue;
        }

        // Properties are indented lines: `key = "value"`
        if let Some((key, value)) = parse_property(trimmed) {
            match key {
                "application.name" => {
                    tracing::trace!(index = entry.index, app_name = %value, "parsed application.name");
                    entry.app_name = Some(value);
                }
                "application.process.binary" => {
                    tracing::trace!(index = entry.index, binary = %value, "parsed application.process.binary");
                    entry.app_binary = Some(value);
                }
                "application.icon_name" => {
                    tracing::trace!(index = entry.index, icon = %value, "parsed application.icon_name");
                    entry.icon_name = Some(value);
                }
                "media.name" => {
                    tracing::trace!(index = entry.index, media_name = %value, "parsed media.name");
                    entry.media_name = Some(value);
                }
                _ => {}
            }
        }
    }

    // Flush the last entry.
    if let Some(entry) = current {
        if let Some(app) = entry.into_application() {
            apps.push(app);
        }
    }

    apps
}

/// Parse "Sink Input #123" from a header line. Returns the index.
fn parse_sink_input_header(line: &str) -> Option<u32> {
    let line = line.trim();
    let rest = line.strip_prefix("Sink Input #")?;
    rest.parse::<u32>().ok()
}

/// Parse a property line like `application.name = "Firefox"`.
/// Returns `(key, value)` with surrounding quotes stripped from value.
fn parse_property(line: &str) -> Option<(&str, String)> {
    let (key, value) = line.split_once(" = ")?;
    let key = key.trim();
    let value = value.trim();
    // Strip surrounding quotes.
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    Some((key, value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = r#"Sink Input #42
	Driver: protocol-native.c
	State: RUNNING
	Sink: 1
	Volume: front-left: 65536 / 100% / 0.00 dB,   front-right: 65536 / 100% / 0.00 dB
	Properties:
		media.name = "Playback"
		application.name = "Firefox"
		application.process.binary = "firefox"
		application.icon_name = "firefox"

Sink Input #99
	Driver: protocol-native.c
	State: RUNNING
	Sink: 1
	Properties:
		media.name = "Loopback to channel-1"
		application.name = "PulseAudio Volume Control"
		application.process.binary = "pavucontrol"

Sink Input #101
	Driver: protocol-native.c
	State: RUNNING
	Sink: 2
	Properties:
		media.name = "Music"
		application.process.binary = "spotify"
"#;

    #[test]
    fn parses_firefox_entry() {
        let apps = parse_sink_inputs(SAMPLE_OUTPUT);
        let firefox = apps.iter().find(|a| a.name == "Firefox");
        assert!(firefox.is_some(), "Firefox should be detected");
        let ff = firefox.unwrap();
        assert_eq!(ff.id, 42);
        assert_eq!(ff.stream_index, 42);
        assert_eq!(ff.binary, "firefox");
        assert_eq!(ff.icon_name.as_deref(), Some("firefox"));
        assert!(ff.channel.is_none());
    }

    #[test]
    fn filters_loopback_streams() {
        let apps = parse_sink_inputs(SAMPLE_OUTPUT);
        let loopback = apps.iter().find(|a| a.name == "PulseAudio Volume Control");
        assert!(loopback.is_none(), "loopback streams should be filtered out");
    }

    #[test]
    fn filters_entries_without_app_name() {
        let apps = parse_sink_inputs(SAMPLE_OUTPUT);
        let spotify = apps.iter().find(|a| a.binary == "spotify");
        assert!(
            spotify.is_none(),
            "entries without application.name should be filtered out"
        );
    }

    #[test]
    fn empty_output_returns_empty() {
        let apps = parse_sink_inputs("");
        assert!(apps.is_empty());
    }
}
