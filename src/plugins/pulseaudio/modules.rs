use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::error::{OsgError, Result};

pub struct ModuleManager {
    loaded_modules: Vec<u32>,
}

impl ModuleManager {
    pub fn new() -> Self {
        Self {
            loaded_modules: Vec::new(),
        }
    }

    pub fn create_null_sink(&mut self, name: &str, description: &str) -> Result<u32> {
        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-null-sink",
                &format!("sink_name={name}"),
                &format!("sink_properties=device.description={description}"),
            ])
            .output()
            .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OsgError::ModuleLoadFailed(format!(
                "module-null-sink '{name}': {stderr}"
            )));
        }

        let module_id = parse_module_id(&output.stdout)?;
        self.loaded_modules.push(module_id);
        Ok(module_id)
    }

    pub fn create_loopback(
        &mut self,
        source_monitor: &str,
        sink: &str,
        latency_ms: u32,
    ) -> Result<u32> {
        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-loopback",
                &format!("source={source_monitor}"),
                &format!("sink={sink}"),
                &format!("latency_msec={latency_ms}"),
            ])
            .output()
            .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OsgError::ModuleLoadFailed(format!(
                "module-loopback {source_monitor} -> {sink}: {stderr}"
            )));
        }

        let module_id = parse_module_id(&output.stdout)?;
        self.loaded_modules.push(module_id);
        Ok(module_id)
    }

    pub fn unload_module(&mut self, module_id: u32) -> Result<()> {
        let output = Command::new("pactl")
            .args(["unload-module", &module_id.to_string()])
            .output()
            .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OsgError::PulseAudio(format!(
                "unload-module {module_id}: {stderr}"
            )));
        }

        self.loaded_modules.retain(|&id| id != module_id);
        Ok(())
    }

    pub fn unload_all(&mut self) {
        let ids: Vec<u32> = self.loaded_modules.clone();
        for id in ids {
            if let Err(e) = self.unload_module(id) {
                tracing::warn!("Failed to unload module {id}: {e}");
            }
        }
    }

    /// Find the sink-input index created by a loopback module.
    ///
    /// Retries up to 3 times with 100ms delay between attempts because
    /// PipeWire can take a moment to register the sink-input after the
    /// module is loaded.
    pub fn find_loopback_sink_input(&self, module_id: u32) -> Result<Option<u32>> {
        let target = format!("\"{}\"", module_id);

        for attempt in 0..3 {
            if attempt > 0 {
                thread::sleep(Duration::from_millis(100));
            }

            let output = Command::new("pactl")
                .args(["list", "sink-inputs"])
                .output()
                .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(OsgError::PulseAudio(format!(
                    "list sink-inputs: {stderr}"
                )));
            }

            let text = String::from_utf8_lossy(&output.stdout);

            if let Some(idx) = parse_sink_input_for_module(&text, &target) {
                return Ok(Some(idx));
            }
        }

        Ok(None)
    }

    pub fn set_sink_input_volume(&self, sink_input_idx: u32, volume: f32) -> Result<()> {
        let percent = (volume * 100.0) as u32;
        let output = Command::new("pactl")
            .args([
                "set-sink-input-volume",
                &sink_input_idx.to_string(),
                &format!("{percent}%"),
            ])
            .output()
            .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OsgError::PulseAudio(format!(
                "set-sink-input-volume {sink_input_idx}: {stderr}"
            )));
        }

        Ok(())
    }

    pub fn set_sink_input_mute(&self, sink_input_idx: u32, muted: bool) -> Result<()> {
        let mute_val = if muted { "1" } else { "0" };
        let output = Command::new("pactl")
            .args([
                "set-sink-input-mute",
                &sink_input_idx.to_string(),
                mute_val,
            ])
            .output()
            .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OsgError::PulseAudio(format!(
                "set-sink-input-mute {sink_input_idx}: {stderr}"
            )));
        }

        Ok(())
    }

    pub fn move_sink_input(&self, sink_input_idx: u32, sink_name: &str) -> Result<()> {
        let output = Command::new("pactl")
            .args([
                "move-sink-input",
                &sink_input_idx.to_string(),
                sink_name,
            ])
            .output()
            .map_err(|e| OsgError::PulseAudio(format!("failed to run pactl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OsgError::PulseAudio(format!(
                "move-sink-input {sink_input_idx} -> {sink_name}: {stderr}"
            )));
        }

        Ok(())
    }
}

/// Parse the module ID from `pactl load-module` stdout (a single integer line).
fn parse_module_id(stdout: &[u8]) -> Result<u32> {
    let text = String::from_utf8_lossy(stdout);
    text.trim()
        .parse::<u32>()
        .map_err(|e| OsgError::ModuleLoadFailed(format!("invalid module id '{text}': {e}")))
}

/// Parse `pactl list sink-inputs` output to find the sink-input index
/// belonging to a specific module.
///
/// Sections look like:
/// ```text
/// Sink Input #42
///     ...
///     Properties:
///         pulse.module.id = "7"
/// ```
fn parse_sink_input_for_module(text: &str, module_id_quoted: &str) -> Option<u32> {
    let mut current_idx: Option<u32> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("Sink Input #") {
            current_idx = rest.parse::<u32>().ok();
        }

        if trimmed.starts_with("pulse.module.id =") {
            if let Some(idx) = current_idx {
                if trimmed.ends_with(module_id_quoted) {
                    return Some(idx);
                }
            }
        }
    }

    None
}
