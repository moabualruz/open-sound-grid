//! WirePlumber CLI helpers for volume, mute, and device enumeration.
//!
//! wpctl is the standard interface for controlling PipeWire audio
//! via WirePlumber (the session manager). It handles all the SPA
//! parameter negotiation internally, making it more reliable than
//! raw pipewire-rs SPA POD construction for dynamic operations.

#[cfg(feature = "pipewire-backend")]
use std::process::Command;

#[cfg(feature = "pipewire-backend")]
use crate::error::{OsgError, Result};

#[cfg(feature = "pipewire-backend")]
use crate::plugin::api::{AudioApplication, HardwareInput, HardwareOutput};

/// Set volume on a PipeWire node by its object ID.
/// Volume range: 0.0 (silent) to 1.0+ (above unity).
#[cfg(feature = "pipewire-backend")]
pub fn set_volume(node_id: u32, volume: f32) -> Result<()> {
    let vol = volume.clamp(0.0, 1.5);
    tracing::debug!(node_id, volume = vol, "wpctl set-volume");
    let output = Command::new("wpctl")
        .args(["set-volume", &node_id.to_string(), &format!("{vol:.3}")])
        .output()
        .map_err(|e| OsgError::PulseAudio(format!("wpctl set-volume failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(node_id, stderr = %stderr, "wpctl set-volume failed");
    }
    Ok(())
}

/// Set mute state on a PipeWire node.
#[cfg(feature = "pipewire-backend")]
pub fn set_mute(node_id: u32, muted: bool) -> Result<()> {
    let val = if muted { "1" } else { "0" };
    tracing::debug!(node_id, muted, "wpctl set-mute");
    let output = Command::new("wpctl")
        .args(["set-mute", &node_id.to_string(), val])
        .output()
        .map_err(|e| OsgError::PulseAudio(format!("wpctl set-mute failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(node_id, stderr = %stderr, "wpctl set-mute failed");
    }
    Ok(())
}

/// Get the current volume of a PipeWire node (0.0–1.0+).
#[cfg(feature = "pipewire-backend")]
pub fn get_volume(node_id: u32) -> Result<f32> {
    let output = Command::new("wpctl")
        .args(["get-volume", &node_id.to_string()])
        .output()
        .map_err(|e| OsgError::PulseAudio(format!("wpctl get-volume failed: {e}")))?;
    let text = String::from_utf8_lossy(&output.stdout);
    // Output: "Volume: 0.75" or "Volume: 0.75 [MUTED]"
    text.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<f32>().ok())
        .ok_or_else(|| OsgError::PulseAudio(format!("failed to parse wpctl volume: {text}")))
}

/// Resolve a PipeWire node's global ID by searching for its description in wpctl status.
/// Returns the HIGHEST matching global ID (most recently created node).
#[cfg(feature = "pipewire-backend")]
pub fn resolve_node_id_by_name(search: &str) -> Option<u32> {
    let output = Command::new("wpctl").args(["status"]).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let search_normalized = search.replace('_', " ").to_lowercase();

    let mut best_match: Option<u32> = None;

    for line in text.lines() {
        if let Some((name, id)) = parse_wpctl_device_line(line) {
            let name_normalized = name.replace('_', " ").to_lowercase();
            if name_normalized == search_normalized
                || name_normalized.contains(&search_normalized)
                || search_normalized.contains(&name_normalized)
            {
                // Take the highest ID (most recently created)
                if best_match.map_or(true, |prev| id > prev) {
                    best_match = Some(id);
                }
            }
        }
    }

    if let Some(id) = best_match {
        tracing::debug!(search, global_id = id, "resolved PW node global ID");
    } else {
        tracing::warn!(search, "could not resolve PW node global ID from wpctl status");
    }
    best_match
}

/// Enumerate hardware audio sinks (output devices) via wpctl status.
#[cfg(feature = "pipewire-backend")]
pub fn list_hardware_outputs() -> Vec<HardwareOutput> {
    let output = match Command::new("wpctl").args(["status"]).output() {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(err = %e, "wpctl status failed");
            return vec![];
        }
    };
    let text = String::from_utf8_lossy(&output.stdout);
    parse_sinks_from_status(&text)
}

/// Enumerate hardware audio sources (input devices) via wpctl status.
#[cfg(feature = "pipewire-backend")]
pub fn list_hardware_inputs() -> Vec<HardwareInput> {
    let output = match Command::new("wpctl").args(["status"]).output() {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(err = %e, "wpctl status failed");
            return vec![];
        }
    };
    let text = String::from_utf8_lossy(&output.stdout);
    parse_sources_from_status(&text)
}

/// Enumerate running audio streams (applications) via wpctl status.
#[cfg(feature = "pipewire-backend")]
pub fn list_applications() -> Vec<AudioApplication> {
    let output = match Command::new("pw-cli").args(["ls", "Node"]).output() {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(err = %e, "pw-cli ls Node failed");
            return vec![];
        }
    };
    let text = String::from_utf8_lossy(&output.stdout);
    parse_stream_nodes(&text)
}

/// Move a PipeWire stream to a specific sink node using pw-cli.
/// This is equivalent to PA's move-sink-input.
#[cfg(feature = "pipewire-backend")]
pub fn move_stream_to_sink(stream_id: u32, sink_node_id: u32) -> Result<()> {
    tracing::debug!(stream_id, sink_node_id, "pw-metadata: setting target.node");
    // Use pw-metadata to set the target node for a stream
    let output = Command::new("pw-metadata")
        .args([
            &stream_id.to_string(),
            "target.object",
            &format!("{{\"name\": \"default\", \"id\": {sink_node_id}}}"),
            "Spa:Id:Object",
        ])
        .output()
        .map_err(|e| OsgError::PulseAudio(format!("pw-metadata failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(stream_id, stderr = %stderr, "pw-metadata set target.object failed");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "pipewire-backend")]
fn parse_sinks_from_status(text: &str) -> Vec<HardwareOutput> {
    let mut outputs = vec![];
    let mut in_sinks = false;
    let mut next_id: u32 = 1;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("├─ Sinks:") || trimmed.starts_with("│  ├─ Sinks:") {
            in_sinks = true;
            continue;
        }
        if in_sinks
            && (trimmed.starts_with("├─ ") || trimmed.starts_with("└─ "))
            && !trimmed.contains("Sinks:")
        {
            in_sinks = false;
            continue;
        }
        if in_sinks {
            // Lines like: "│  *   55. RODECaster Duo Pro  [vol: 1.00]"
            // or:         "│     165. OSG_Music_Channel   [vol: 1.00]"
            if let Some(entry) = parse_wpctl_device_line(trimmed) {
                // Skip OSG virtual sinks
                if entry.0.starts_with("OSG_") || entry.0.starts_with("osg_") || entry.0.starts_with("OSG ") {
                    continue;
                }
                outputs.push(HardwareOutput {
                    id: next_id,
                    name: entry.0.clone(),
                    description: entry.0.clone(),
                    device_id: entry.1.to_string(), // PW node ID as string
                });
                next_id += 1;
            }
        }
    }
    tracing::debug!(count = outputs.len(), "parsed hardware outputs from wpctl status");
    outputs
}

#[cfg(feature = "pipewire-backend")]
fn parse_sources_from_status(text: &str) -> Vec<HardwareInput> {
    let mut inputs = vec![];
    let mut in_sources = false;
    let mut next_id: u32 = 1;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("├─ Sources:") || trimmed.starts_with("│  ├─ Sources:") {
            in_sources = true;
            continue;
        }
        if in_sources
            && (trimmed.starts_with("├─ ") || trimmed.starts_with("└─ "))
            && !trimmed.contains("Sources:")
        {
            in_sources = false;
            continue;
        }
        if in_sources {
            if let Some(entry) = parse_wpctl_device_line(trimmed) {
                inputs.push(HardwareInput {
                    id: next_id,
                    name: entry.0.clone(),
                    description: entry.0.clone(),
                    device_id: entry.1.to_string(),
                });
                next_id += 1;
            }
        }
    }
    tracing::debug!(count = inputs.len(), "parsed hardware inputs from wpctl status");
    inputs
}

/// Parse a wpctl status line like "│  *   55. RODECaster Duo Pro  [vol: 1.00]"
/// Returns (name, pw_node_id).
#[cfg(feature = "pipewire-backend")]
fn parse_wpctl_device_line(line: &str) -> Option<(String, u32)> {
    // Strip │ and * markers
    let cleaned = line.replace('│', "").replace('*', "");
    let cleaned = cleaned.trim();
    // Format: "55. RODECaster Duo Pro  [vol: 1.00]"
    let dot_pos = cleaned.find('.')?;
    let id: u32 = cleaned[..dot_pos].trim().parse().ok()?;
    let rest = cleaned[dot_pos + 1..].trim();
    // Strip [vol: ...] suffix
    let name = if let Some(bracket) = rest.find('[') {
        rest[..bracket].trim()
    } else {
        rest.trim()
    };
    if name.is_empty() {
        return None;
    }
    Some((name.to_string(), id))
}

/// Parse pw-cli ls Node output for audio playback streams.
#[cfg(feature = "pipewire-backend")]
fn parse_stream_nodes(text: &str) -> Vec<AudioApplication> {
    let mut apps = vec![];
    let mut current_id: Option<u32> = None;
    let mut current_props: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut next_app_id: u32 = 1;

    for line in text.lines() {
        let trimmed = line.trim();
        // New object: "id 42, type PipeWire:Interface:Node/3, ..."
        if trimmed.starts_with("id ") && trimmed.contains("type PipeWire:Interface:Node") {
            // Flush previous
            if let Some(id) = current_id {
                if let Some(app) = build_app_from_props(id, &current_props, next_app_id) {
                    apps.push(app);
                    next_app_id += 1;
                }
            }
            current_id = trimmed
                .split(',')
                .next()
                .and_then(|s| s.strip_prefix("id "))
                .and_then(|s| s.trim().parse().ok());
            current_props.clear();
        }
        // Property: "key = value"
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim().trim_matches('"');
            let value = value.trim().trim_matches('"');
            current_props.insert(key.to_string(), value.to_string());
        }
    }
    // Flush last
    if let Some(id) = current_id {
        if let Some(app) = build_app_from_props(id, &current_props, next_app_id) {
            apps.push(app);
        }
    }

    tracing::debug!(count = apps.len(), "parsed audio applications from pw-cli");
    apps
}

#[cfg(feature = "pipewire-backend")]
fn build_app_from_props(
    pw_id: u32,
    props: &std::collections::HashMap<String, String>,
    app_id: u32,
) -> Option<AudioApplication> {
    let media_class = props.get("media.class")?;
    if media_class != "Stream/Output/Audio" && media_class != "Stream/Audio/Playback" {
        return None;
    }
    let name = props
        .get("application.name")
        .or_else(|| props.get("node.name"))
        .cloned()
        .unwrap_or_else(|| format!("stream-{pw_id}"));
    let binary = props
        .get("application.process.binary")
        .cloned()
        .unwrap_or_default();

    Some(AudioApplication {
        id: app_id,
        name,
        binary,
        icon_name: props.get("application.icon-name").cloned(),
        icon_path: None,
        stream_index: pw_id, // Use PW node ID as stream_index
        channel: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wpctl_device_line() {
        let line = "│  *   55. RODECaster Duo Pro                  [vol: 1.00]";
        let result = parse_wpctl_device_line(line);
        assert!(result.is_some());
        let (name, id) = result.unwrap();
        assert_eq!(name, "RODECaster Duo Pro");
        assert_eq!(id, 55);
    }

    #[test]
    fn test_parse_wpctl_skips_empty() {
        let line = "│      .   [vol: 1.00]";
        assert!(parse_wpctl_device_line(line).is_none());
    }

    #[test]
    fn test_parse_wpctl_osg_sink() {
        let line = "│     165. OSG_Music_Channel                   [vol: 1.00]";
        let result = parse_wpctl_device_line(line);
        assert!(result.is_some());
        let (name, id) = result.unwrap();
        assert_eq!(name, "OSG_Music_Channel");
        assert_eq!(id, 165);
    }
}
