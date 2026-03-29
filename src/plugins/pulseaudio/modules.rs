use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::error::{OsgError, Result};

use super::connection::PulseConnection;
use super::introspect;
use super::introspect_control;

pub struct ModuleManager {
    loaded_modules: Vec<u32>,
    /// Whether PipeWire is the active audio server (enables pw-link routing).
    pipewire_detected: bool,
    /// pw-link connections tracked as (source_port, sink_port) for teardown.
    pw_links: Vec<(String, String)>,
}

impl ModuleManager {
    pub fn new() -> Self {
        let pipewire_detected = detect_pipewire();
        if pipewire_detected {
            tracing::info!("PipeWire detected — will use pw-link for zero-latency routing");
        } else {
            tracing::info!("Native PulseAudio — using module-loopback for routing");
        }
        Self {
            loaded_modules: Vec::new(),
            pipewire_detected,
            pw_links: Vec::new(),
        }
    }

    /// Whether PipeWire is the active audio server.
    pub fn is_pipewire(&self) -> bool {
        self.pipewire_detected
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
            // PipeWire PA-compat truncates device.description at first space.
            // Replace spaces with underscores for a clean, distinguishable name.
            let desc_safe = description.replace(' ', "_");
            let args = format!("sink_name={name} sink_properties=device.description={desc_safe}");
            introspect_control::load_module_sync(conn, "module-null-sink", &args)?
        } else {
            tracing::debug!(sink_name = %name, "no PA connection — falling back to pactl for null sink");
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-null-sink",
                    &format!("sink_name={name}"),
                    &format!("sink_properties=device.description={}", description.replace(' ', "_")),
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

    /// Create a route connection between source monitor and sink via module-loopback.
    ///
    /// On PipeWire systems, the loopback module runs through pipewire-pulse which
    /// already provides lower latency than native PulseAudio. Volume control is
    /// via set-sink-input-volume on the loopback's sink-input (with PipeWire
    /// fallback matching when owner_module is unavailable).
    #[tracing::instrument(skip(self, conn))]
    pub fn create_loopback(
        &mut self,
        conn: Option<&mut PulseConnection>,
        source_monitor: &str,
        sink: &str,
        latency_ms: u32,
    ) -> Result<u32> {
        // PipeWire uses lower effective latency for module-loopback than the
        // requested latency_msec — the PA compat layer translates to PW graph
        // quantum which is typically 1024/48000 ≈ 21ms regardless of setting.
        let effective_latency = if self.pipewire_detected { 1 } else { latency_ms };
        tracing::debug!(
            source = %source_monitor, sink = %sink,
            requested_latency_ms = latency_ms, effective_latency_ms = effective_latency,
            pipewire = self.pipewire_detected,
            "creating loopback"
        );

        let module_id = if let Some(conn) = conn {
            let args = format!("source={source_monitor} sink={sink} latency_msec={effective_latency}");
            introspect_control::load_module_sync(conn, "module-loopback", &args)?
        } else {
            tracing::debug!(source = %source_monitor, sink = %sink, "no PA connection — falling back to pactl for loopback");
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-loopback",
                    &format!("source={source_monitor}"),
                    &format!("sink={sink}"),
                    &format!("latency_msec={effective_latency}"),
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
            introspect_control::unload_module_sync(conn, module_id)?;
        } else {
            tracing::debug!(
                module_id = module_id,
                "no PA connection — falling back to pactl for unload"
            );
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
    /// Find the sink-input index created by a loopback module.
    ///
    /// Primary: match by `owner_module`. Fallback (PipeWire): match by
    /// `target_sink_name` — the PA sink the loopback writes to. PipeWire
    /// often sets `owner_module` to None, making the primary lookup fail.
    ///
    /// `target_sink_name` is the mix sink (e.g. "osg_Main/Monitor_mix")
    /// that the loopback was created with.
    #[tracing::instrument(skip(self, conn))]
    pub fn find_loopback_sink_input(
        &self,
        conn: Option<&mut PulseConnection>,
        module_id: u32,
    ) -> Result<Option<u32>> {
        self.find_loopback_sink_input_with_fallback(conn, module_id, None)
    }

    /// Same as `find_loopback_sink_input` but with an optional target sink
    /// name for PipeWire fallback matching.
    #[tracing::instrument(skip(self, conn))]
    pub fn find_loopback_sink_input_with_fallback(
        &self,
        conn: Option<&mut PulseConnection>,
        module_id: u32,
        target_sink_name: Option<&str>,
    ) -> Result<Option<u32>> {
        if let Some(conn) = conn {
            // Resolve target sink name to a sink index for fallback matching
            let target_sink_idx = target_sink_name.and_then(|name| {
                introspect::resolve_sink_index_by_name(conn, name).ok().flatten()
            });
            if let Some(idx) = target_sink_idx {
                tracing::debug!(
                    module_id, target_sink_name = ?target_sink_name, target_sink_idx = idx,
                    "resolved target sink for PipeWire fallback"
                );
            }

            for attempt in 0..5 {
                if attempt > 0 {
                    tracing::debug!(
                        module_id = module_id,
                        attempt = attempt,
                        "retrying sink-input lookup via introspect"
                    );
                    thread::sleep(Duration::from_millis(150));
                }

                match introspect::find_sink_input_by_module_or_sink_sync(
                    conn, module_id, target_sink_idx,
                )? {
                    Some(idx) => {
                        tracing::debug!(
                            module_id = module_id,
                            sink_input_idx = idx,
                            attempt = attempt,
                            "found sink-input via introspect"
                        );
                        return Ok(Some(idx));
                    }
                    None => {
                        tracing::debug!(
                            module_id = module_id,
                            attempt = attempt,
                            "sink-input not found yet via introspect"
                        );
                    }
                }
            }
            tracing::warn!(
                module_id = module_id,
                "sink-input not found after 5 introspect attempts (750ms total)"
            );
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
            return introspect_control::set_sink_input_volume_sync(conn, sink_input_idx, volume);
        }

        let percent = (volume * 100.0) as u32;
        tracing::debug!(
            sink_input_idx = sink_input_idx,
            volume = volume,
            percent = percent,
            "setting sink-input volume via pactl (fallback)"
        );

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

    /// Set per-channel stereo volume on a sink-input using separate L/R values.
    ///
    /// Uses `pactl set-sink-input-volume` with two percentage arguments
    /// (one per channel), which PA interprets as left/right.
    #[tracing::instrument(skip(self, conn))]
    pub fn set_sink_input_stereo_volume(
        &self,
        conn: Option<&mut PulseConnection>,
        sink_input_idx: u32,
        left: f32,
        right: f32,
    ) -> Result<()> {
        if let Some(conn) = conn {
            // Use introspect API with 2-channel ChannelVolumes
            return introspect_control::set_sink_input_stereo_volume_sync(
                conn,
                sink_input_idx,
                left,
                right,
            );
        }

        let left_pct = (left * 100.0) as u32;
        let right_pct = (right * 100.0) as u32;
        tracing::debug!(
            sink_input_idx, left, right, left_pct, right_pct,
            "setting sink-input stereo volume via pactl (fallback)"
        );

        let output = Command::new("pactl")
            .args([
                "set-sink-input-volume",
                &sink_input_idx.to_string(),
                &format!("{left_pct}%"),
                &format!("{right_pct}%"),
            ])
            .output()
            .map_err(|e| {
                tracing::error!(sink_input_idx, err = %e, "pactl set-sink-input-volume (stereo) failed");
                OsgError::PulseAudio(format!("failed to run pactl: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(sink_input_idx, stderr = %stderr, "set-sink-input-volume (stereo) failed");
            return Err(OsgError::PulseAudio(format!(
                "set-sink-input-volume stereo {sink_input_idx}: {stderr}"
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
            return introspect_control::set_sink_input_mute_sync(conn, sink_input_idx, muted);
        }

        let mute_val = if muted { "1" } else { "0" };
        tracing::debug!(
            sink_input_idx = sink_input_idx,
            muted = muted,
            "setting sink-input mute via pactl (fallback)"
        );

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
            return introspect_control::move_sink_input_sync(conn, sink_input_idx, sink_name);
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
                tracing::debug!(
                    module_id = module_id,
                    attempt = attempt,
                    "retrying sink-input lookup via pactl"
                );
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
                return Err(OsgError::PulseAudio(format!("list sink-inputs: {stderr}")));
            }

            let text = String::from_utf8_lossy(&output.stdout);
            tracing::trace!(
                module_id = module_id,
                output_len = text.len(),
                "pactl list sink-inputs output received"
            );

            if let Some(idx) = parse_sink_input_for_module(&text, &target) {
                tracing::debug!(
                    module_id = module_id,
                    sink_input_idx = idx,
                    attempt = attempt,
                    "found sink-input via pactl"
                );
                return Ok(Some(idx));
            }
        }

        tracing::warn!(
            module_id = module_id,
            "sink-input not found after 3 pactl attempts"
        );
        Ok(None)
    }

    // -----------------------------------------------------------------------
    // pw-link routing (PipeWire zero-latency direct port connections)
    // -----------------------------------------------------------------------

    /// Create direct PipeWire port links between a source monitor and a sink.
    /// Links both FL and FR channels for stereo.
    fn create_pwlink(&mut self, source_monitor: &str, sink: &str) -> Result<()> {
        // pw-link uses port names: {node_name}:monitor_FL, {node_name}:playback_FL
        // source_monitor is like "osg_Music_ch.monitor" — strip the .monitor suffix
        let source_node = source_monitor.strip_suffix(".monitor").unwrap_or(source_monitor);

        let pairs = [
            (format!("{source_node}:monitor_FL"), format!("{sink}:playback_FL")),
            (format!("{source_node}:monitor_FR"), format!("{sink}:playback_FR")),
        ];

        for (src_port, sink_port) in &pairs {
            let output = Command::new("pw-link")
                .args([src_port.as_str(), sink_port.as_str()])
                .output()
                .map_err(|e| OsgError::PulseAudio(format!("pw-link failed to execute: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // "File exists" means already linked — that's OK
                if !stderr.contains("File exists") {
                    tracing::warn!(
                        src = %src_port, sink = %sink_port, stderr = %stderr,
                        "pw-link failed"
                    );
                    return Err(OsgError::PulseAudio(format!(
                        "pw-link {src_port} -> {sink_port}: {stderr}"
                    )));
                }
                tracing::debug!(src = %src_port, sink = %sink_port, "pw-link already exists");
            }
            self.pw_links.push((src_port.clone(), sink_port.clone()));
        }

        tracing::debug!(
            source_node = %source_node, sink = %sink,
            "pw-link stereo pair created (FL + FR)"
        );
        Ok(())
    }

    /// Tear down pw-link connections for a specific source or sink node.
    pub fn destroy_pwlinks_for(&mut self, node_name: &str) {
        let to_remove: Vec<(String, String)> = self.pw_links
            .iter()
            .filter(|(src, sink)| src.starts_with(node_name) || sink.starts_with(node_name))
            .cloned()
            .collect();

        for (src, sink) in &to_remove {
            let output = Command::new("pw-link")
                .args(["-d", src.as_str(), sink.as_str()])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    tracing::debug!(src = %src, sink = %sink, "pw-link removed");
                }
                _ => {
                    tracing::debug!(src = %src, sink = %sink, "pw-link remove failed (may already be gone)");
                }
            }
        }

        self.pw_links.retain(|(src, sink)| {
            !src.starts_with(node_name) && !sink.starts_with(node_name)
        });
    }

    /// Destroy all pw-link connections (cleanup on shutdown).
    pub fn destroy_all_pwlinks(&mut self) {
        for (src, sink) in &self.pw_links {
            let _ = Command::new("pw-link")
                .args(["-d", src.as_str(), sink.as_str()])
                .output();
        }
        let count = self.pw_links.len();
        self.pw_links.clear();
        if count > 0 {
            tracing::info!(count, "all pw-links destroyed");
        }
    }

    /// Set volume on a PipeWire node via wpctl (used when pw-link routing is active).
    pub fn wpctl_set_volume(&self, node_name: &str, volume: f32) -> Result<()> {
        if !self.pipewire_detected {
            return Ok(());
        }
        // Find the node's wpctl ID by name
        let output = Command::new("wpctl").args(["status"]).output()
            .map_err(|e| OsgError::PulseAudio(format!("wpctl status failed: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let search = node_name.replace('_', " ").to_lowercase();

        for line in text.lines() {
            let cleaned = line.replace('│', "").replace('*', "");
            let cleaned = cleaned.trim();
            if let Some(dot_pos) = cleaned.find('.') {
                if let Ok(id) = cleaned[..dot_pos].trim().parse::<u32>() {
                    let rest = cleaned[dot_pos + 1..].trim();
                    let name = if let Some(bracket) = rest.find('[') {
                        rest[..bracket].trim()
                    } else {
                        rest
                    };
                    if name.replace('_', " ").to_lowercase().contains(&search) {
                        let vol = volume.clamp(0.0, 1.5);
                        let out = Command::new("wpctl")
                            .args(["set-volume", &id.to_string(), &format!("{vol:.3}")])
                            .output();
                        if let Ok(o) = out {
                            if o.status.success() {
                                tracing::debug!(node_name, wpctl_id = id, volume = vol, "wpctl set-volume applied");
                            }
                        }
                        return Ok(());
                    }
                }
            }
        }
        tracing::warn!(node_name, "wpctl_set_volume: node not found in wpctl status");
        Ok(())
    }

    /// Set mute on a PipeWire node via wpctl.
    pub fn wpctl_set_mute(&self, node_name: &str, muted: bool) -> Result<()> {
        if !self.pipewire_detected {
            return Ok(());
        }
        let output = Command::new("wpctl").args(["status"]).output()
            .map_err(|e| OsgError::PulseAudio(format!("wpctl status failed: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let search = node_name.replace('_', " ").to_lowercase();

        for line in text.lines() {
            let cleaned = line.replace('│', "").replace('*', "");
            let cleaned = cleaned.trim();
            if let Some(dot_pos) = cleaned.find('.') {
                if let Ok(id) = cleaned[..dot_pos].trim().parse::<u32>() {
                    let rest = cleaned[dot_pos + 1..].trim();
                    let name = if let Some(bracket) = rest.find('[') {
                        rest[..bracket].trim()
                    } else {
                        rest
                    };
                    if name.replace('_', " ").to_lowercase().contains(&search) {
                        let val = if muted { "1" } else { "0" };
                        let out = Command::new("wpctl")
                            .args(["set-mute", &id.to_string(), val])
                            .output();
                        if let Ok(o) = out {
                            if o.status.success() {
                                tracing::debug!(node_name, wpctl_id = id, muted, "wpctl set-mute applied");
                            }
                        }
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

/// Detect whether PipeWire is the active audio server (not just PA).
fn detect_pipewire() -> bool {
    // Check pactl info — PipeWire's PA compat reports "PulseAudio (on PipeWire X.Y.Z)"
    let output = Command::new("pactl").args(["info"]).output();
    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let is_pw = text.contains("on PipeWire");
            tracing::debug!(
                is_pipewire = is_pw,
                "PipeWire detection via pactl info"
            );
            is_pw
        }
        _ => {
            tracing::debug!("pactl info failed — assuming native PulseAudio");
            false
        }
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
