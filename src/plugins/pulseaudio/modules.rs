use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::error::{OsgError, Result};

use super::connection::PulseConnection;
use super::introspect;

pub struct ModuleManager {
    loaded_modules: Vec<u32>,
}

impl ModuleManager {
    pub fn new() -> Self {
        Self {
            loaded_modules: Vec::new(),
        }
    }

    /// Return the number of currently tracked loaded modules.
    pub fn module_count(&self) -> usize {
        self.loaded_modules.len()
    }

    #[tracing::instrument(skip(self, conn))]
    pub fn create_null_sink(
        &mut self,
        conn: Option<&mut PulseConnection>,
        name: &str,
        description: &str,
    ) -> Result<u32> {
        tracing::debug!(sink_name = %name, description = %description, "creating null sink");

        let module_id = if let Some(conn) = conn {
            let args = format!(
                "sink_name={name} sink_properties=device.description={description}"
            );
            introspect::load_module_sync(conn, "module-null-sink", &args)?
        } else {
            tracing::debug!(sink_name = %name, "no PA connection — falling back to pactl for null sink");
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-null-sink",
                    &format!("sink_name={name}"),
                    &format!("sink_properties=device.description={description}"),
                ])
                .output()
                .map_err(|e| {
                    tracing::error!(sink_name = %name, err = %e, "pactl load-module failed to execute");
                    OsgError::PulseAudio(format!("failed to run pactl: {e}"))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(sink_name = %name, stderr = %stderr, "module-null-sink load failed");
                return Err(OsgError::ModuleLoadFailed(format!(
                    "module-null-sink '{name}': {stderr}"
                )));
            }

            parse_module_id(&output.stdout)?
        };

        self.loaded_modules.push(module_id);
        tracing::debug!(sink_name = %name, module_id = module_id, "null sink created");
        Ok(module_id)
    }

    #[tracing::instrument(skip(self, conn))]
    pub fn create_loopback(
        &mut self,
        conn: Option<&mut PulseConnection>,
        source_monitor: &str,
        sink: &str,
        latency_ms: u32,
    ) -> Result<u32> {
        tracing::debug!(source = %source_monitor, sink = %sink, latency_ms = latency_ms, "creating loopback");

        let module_id = if let Some(conn) = conn {
            let args =
                format!("source={source_monitor} sink={sink} latency_msec={latency_ms}");
            introspect::load_module_sync(conn, "module-loopback", &args)?
        } else {
            tracing::debug!(source = %source_monitor, sink = %sink, "no PA connection — falling back to pactl for loopback");
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-loopback",
                    &format!("source={source_monitor}"),
                    &format!("sink={sink}"),
                    &format!("latency_msec={latency_ms}"),
                ])
                .output()
                .map_err(|e| {
                    tracing::error!(source = %source_monitor, sink = %sink, err = %e, "pactl load-module loopback failed to execute");
                    OsgError::PulseAudio(format!("failed to run pactl: {e}"))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(source = %source_monitor, sink = %sink, stderr = %stderr, "module-loopback load failed");
                return Err(OsgError::ModuleLoadFailed(format!(
                    "module-loopback {source_monitor} -> {sink}: {stderr}"
                )));
            }

            parse_module_id(&output.stdout)?
        };

        self.loaded_modules.push(module_id);
        tracing::debug!(source = %source_monitor, sink = %sink, module_id = module_id, "loopback created");
        Ok(module_id)
    }

    #[tracing::instrument(skip(self, conn))]
    pub fn unload_module(
        &mut self,
        conn: Option<&mut PulseConnection>,
        module_id: u32,
    ) -> Result<()> {
        tracing::debug!(module_id = module_id, "unloading module");

        if let Some(conn) = conn {
            introspect::unload_module_sync(conn, module_id)?;
        } else {
            tracing::debug!(module_id = module_id, "no PA connection — falling back to pactl for unload");
            let output = Command::new("pactl")
                .args(["unload-module", &module_id.to_string()])
                .output()
                .map_err(|e| {
                    tracing::error!(module_id = module_id, err = %e, "pactl unload-module failed to execute");
                    OsgError::PulseAudio(format!("failed to run pactl: {e}"))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(module_id = module_id, stderr = %stderr, "unload-module failed");
                return Err(OsgError::PulseAudio(format!(
                    "unload-module {module_id}: {stderr}"
                )));
            }
        }

        self.loaded_modules.retain(|&id| id != module_id);
        tracing::debug!(module_id = module_id, "module unloaded");
        Ok(())
    }

    pub fn unload_all(&mut self, conn: Option<&mut PulseConnection>) {
        let ids: Vec<u32> = self.loaded_modules.clone();
        tracing::info!(count = ids.len(), "unloading all modules");
        // We need to iterate with conn — consume and re-borrow by raw pointer to avoid
        // repeated mutable borrows in a loop.  Safe because each call is sequential.
        match conn {
            Some(c) => {
                for id in ids {
                    if let Err(e) = self.unload_module(Some(c), id) {
                        tracing::warn!(module_id = id, err = %e, "failed to unload module during cleanup");
                    }
                }
            }
            None => {
                for id in ids {
                    if let Err(e) = self.unload_module(None, id) {
                        tracing::warn!(module_id = id, err = %e, "failed to unload module during cleanup");
                    }
                }
            }
        }
    }

    /// Find the sink-input index created by a loopback module.
    ///
    /// When `conn` is provided this uses the libpulse introspect API.
    /// Retries up to 3 times with 100 ms delay because PipeWire can take a
    /// moment to register the sink-input after the module is loaded.
    #[tracing::instrument(skip(self, conn))]
    pub fn find_loopback_sink_input(
        &self,
        conn: Option<&mut PulseConnection>,
        module_id: u32,
    ) -> Result<Option<u32>> {
        if let Some(conn) = conn {
            for attempt in 0..3 {
                if attempt > 0 {
                    tracing::debug!(module_id = module_id, attempt = attempt, "retrying sink-input lookup via introspect");
                    thread::sleep(Duration::from_millis(100));
                }

                match introspect::find_sink_input_by_module_sync(conn, module_id)? {
                    Some(idx) => {
                        tracing::debug!(module_id = module_id, sink_input_idx = idx, attempt = attempt, "found sink-input via introspect");
                        return Ok(Some(idx));
                    }
                    None => {
                        tracing::debug!(module_id = module_id, attempt = attempt, "sink-input not found yet via introspect");
                    }
                }
            }
            tracing::warn!(module_id = module_id, "sink-input not found after 3 introspect attempts");
            Ok(None)
        } else {
            self.find_loopback_sink_input_pactl(module_id)
        }
    }

    #[tracing::instrument(skip(self, conn))]
    pub fn set_sink_input_volume(
        &self,
        conn: Option<&mut PulseConnection>,
        sink_input_idx: u32,
        volume: f32,
    ) -> Result<()> {
        if let Some(conn) = conn {
            return introspect::set_sink_input_volume_sync(conn, sink_input_idx, volume);
        }

        let percent = (volume * 100.0) as u32;
        tracing::debug!(sink_input_idx = sink_input_idx, volume = volume, percent = percent, "setting sink-input volume via pactl (fallback)");

        let output = Command::new("pactl")
            .args([
                "set-sink-input-volume",
                &sink_input_idx.to_string(),
                &format!("{percent}%"),
            ])
            .output()
            .map_err(|e| {
                tracing::error!(sink_input_idx = sink_input_idx, err = %e, "pactl set-sink-input-volume failed to execute");
                OsgError::PulseAudio(format!("failed to run pactl: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(sink_input_idx = sink_input_idx, stderr = %stderr, "set-sink-input-volume failed");
            return Err(OsgError::PulseAudio(format!(
                "set-sink-input-volume {sink_input_idx}: {stderr}"
            )));
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, conn))]
    pub fn set_sink_input_mute(
        &self,
        conn: Option<&mut PulseConnection>,
        sink_input_idx: u32,
        muted: bool,
    ) -> Result<()> {
        if let Some(conn) = conn {
            return introspect::set_sink_input_mute_sync(conn, sink_input_idx, muted);
        }

        let mute_val = if muted { "1" } else { "0" };
        tracing::debug!(sink_input_idx = sink_input_idx, muted = muted, "setting sink-input mute via pactl (fallback)");

        let output = Command::new("pactl")
            .args([
                "set-sink-input-mute",
                &sink_input_idx.to_string(),
                mute_val,
            ])
            .output()
            .map_err(|e| {
                tracing::error!(sink_input_idx = sink_input_idx, err = %e, "pactl set-sink-input-mute failed to execute");
                OsgError::PulseAudio(format!("failed to run pactl: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(sink_input_idx = sink_input_idx, stderr = %stderr, "set-sink-input-mute failed");
            return Err(OsgError::PulseAudio(format!(
                "set-sink-input-mute {sink_input_idx}: {stderr}"
            )));
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, conn))]
    pub fn move_sink_input(
        &self,
        conn: Option<&mut PulseConnection>,
        sink_input_idx: u32,
        sink_name: &str,
    ) -> Result<()> {
        if let Some(conn) = conn {
            return introspect::move_sink_input_sync(conn, sink_input_idx, sink_name);
        }

        tracing::debug!(sink_input_idx = sink_input_idx, sink_name = %sink_name, "moving sink-input via pactl (fallback)");

        let output = Command::new("pactl")
            .args([
                "move-sink-input",
                &sink_input_idx.to_string(),
                sink_name,
            ])
            .output()
            .map_err(|e| {
                tracing::error!(sink_input_idx = sink_input_idx, sink_name = %sink_name, err = %e, "pactl move-sink-input failed to execute");
                OsgError::PulseAudio(format!("failed to run pactl: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(sink_input_idx = sink_input_idx, sink_name = %sink_name, stderr = %stderr, "move-sink-input failed");
            return Err(OsgError::PulseAudio(format!(
                "move-sink-input {sink_input_idx} -> {sink_name}: {stderr}"
            )));
        }

        tracing::debug!(sink_input_idx = sink_input_idx, sink_name = %sink_name, "sink-input moved");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // pactl fallback for find_loopback_sink_input
    // -----------------------------------------------------------------------

    fn find_loopback_sink_input_pactl(&self, module_id: u32) -> Result<Option<u32>> {
        let target = format!("\"{}\"", module_id);

        for attempt in 0..3 {
            if attempt > 0 {
                tracing::debug!(module_id = module_id, attempt = attempt, "retrying sink-input lookup via pactl");
                thread::sleep(Duration::from_millis(100));
            }

            let output = Command::new("pactl")
                .args(["list", "sink-inputs"])
                .output()
                .map_err(|e| {
                    tracing::error!(module_id = module_id, err = %e, "pactl list sink-inputs failed to execute");
                    OsgError::PulseAudio(format!("failed to run pactl: {e}"))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(module_id = module_id, stderr = %stderr, "pactl list sink-inputs returned error");
                return Err(OsgError::PulseAudio(format!(
                    "list sink-inputs: {stderr}"
                )));
            }

            let text = String::from_utf8_lossy(&output.stdout);
            tracing::trace!(module_id = module_id, output_len = text.len(), "pactl list sink-inputs output received");

            if let Some(idx) = parse_sink_input_for_module(&text, &target) {
                tracing::debug!(module_id = module_id, sink_input_idx = idx, attempt = attempt, "found sink-input via pactl");
                return Ok(Some(idx));
            }
        }

        tracing::warn!(module_id = module_id, "sink-input not found after 3 pactl attempts");
        Ok(None)
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
            if let Some(idx) = current_idx {
                tracing::trace!(sink_input_idx = idx, "parsing sink-input section");
            }
        }

        if trimmed.starts_with("pulse.module.id =") {
            if let Some(idx) = current_idx {
                tracing::trace!(sink_input_idx = idx, property = %trimmed, "checking pulse.module.id property");
                if trimmed.ends_with(module_id_quoted) {
                    tracing::trace!(sink_input_idx = idx, module_id_quoted = %module_id_quoted, "matched pulse.module.id property");
                    return Some(idx);
                }
            }
        }
    }

    None
}
