//! Plugin lifecycle: init, set_event_sender, cleanup, Drop, orphan cleanup.

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::mpsc as std_mpsc;

use crate::error::Result;
use crate::plugin::api::*;
use crate::plugin::{API_VERSION, AudioPlugin, PluginCapabilities, PluginInfo};

use super::connection::PulseConnection;
use super::PulseAudioPlugin;
use super::spectrum;

impl Drop for PulseAudioPlugin {
    fn drop(&mut self) {
        // Unload all PA modules (null-sinks + loopbacks) we created
        tracing::info!("dropping PulseAudioPlugin — cleaning up all modules");
        self.modules.unload_all(self.connection.as_mut());

        if let Some(mut child) = self.subscribe_process.take() {
            tracing::debug!("killing subscribe process");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl AudioPlugin for PulseAudioPlugin {
    fn set_latency_ms(&mut self, ms: u32) {
        tracing::info!(latency_ms = ms, "loopback latency configured from settings");
        self.latency_ms = ms;
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pulseaudio",
            name: "PulseAudio",
            version: "0.1.0",
            api_version: API_VERSION,
            os: "linux",
        }
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            can_create_virtual_sinks: true,
            can_route_applications: true,
            can_monitor_peaks: true,
            can_apply_effects: false,
            can_lock_devices: false,
            max_channels: Some(8),
            max_mixes: Some(5),
        }
    }

    fn init(&mut self) -> Result<()> {
        tracing::debug!("initializing PulseAudio plugin");

        // Clean up orphaned OSG sinks from previous crashed sessions
        cleanup_orphaned_osg_modules();

        let conn = PulseConnection::connect()?;
        tracing::info!(
            connected = conn.is_connected(),
            "PulseAudio server reachable via libpulse"
        );
        self.connection = Some(conn);
        // Connection established to verify PA is running.
        // Actual operations use pactl CLI (v0.2 will migrate to libpulse API).
        tracing::info!(
            plugin_id = "pulseaudio",
            version = "0.1.0",
            max_channels = 8,
            max_mixes = 5,
            "PulseAudio plugin initialized"
        );
        Ok(())
    }

    fn set_event_sender(&mut self, tx: std_mpsc::Sender<crate::plugin::PluginThreadMsg>) {
        use crate::plugin::PluginThreadMsg;
        use crate::plugin::manager::PaSubscribeKind;

        tracing::debug!("setting PA event sender — spawning pactl subscribe");
        self.unified_tx = Some(tx.clone());

        // Spawn `pactl subscribe` for real-time PA event notifications
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new("pactl");
        cmd.arg("subscribe")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // Ensure child dies when parent exits (Linux PR_SET_PDEATHSIG)
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        match cmd.spawn() {
            Ok(mut child) => {
                let stdout = child.stdout.take().unwrap();
                // Background thread reads pactl subscribe lines and pushes
                // directly into the plugin thread's unified channel
                std::thread::Builder::new()
                    .name("osg-pa-subscribe".into())
                    .spawn(move || {
                        let reader = std::io::BufReader::new(stdout);
                        for line in reader.lines() {
                            let Ok(line) = line else { break };
                            tracing::trace!(line = %line, "pactl subscribe raw event");
                            // Only react to 'new' and 'remove' events — 'change' events
                            // fire on every volume/mute change and cause a feedback storm
                            // (each SetRouteVolume triggers a state rebuild).
                            let kind = if line.contains("'new' on sink-input")
                                || line.contains("'remove' on sink-input")
                            {
                                Some(PaSubscribeKind::SinkInput)
                            } else if line.contains("'new' on sink")
                                || line.contains("'remove' on sink")
                            {
                                Some(PaSubscribeKind::Sink)
                            } else if line.contains("'new' on source #")
                                || line.contains("'remove' on source #")
                            {
                                // Only new/remove — 'change' on source fires on every volume change
                                Some(PaSubscribeKind::Source)
                            } else {
                                None
                            };
                            if let Some(k) = kind {
                                tracing::debug!(kind = ?k, "PA subscribe event parsed");
                                if tx.send(PluginThreadMsg::PaEvent(k)).is_err() {
                                    break; // unified channel closed
                                }
                            }
                        }
                        tracing::debug!("pactl subscribe reader thread exiting");
                    })
                    .ok();
                self.subscribe_process = Some(child);
                tracing::info!("pactl subscribe started — real-time PA events active");
            }
            Err(e) => {
                tracing::warn!(err = %e, "failed to spawn pactl subscribe — no live PA events");
            }
        }
    }

    fn handle_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        tracing::debug!(cmd = ?cmd, "received plugin command");
        let result = self.dispatch_command(cmd);
        if let Err(ref e) = result {
            tracing::warn!(err = %e, "plugin command returned error");
        }
        result
    }

    fn poll_events(&mut self) -> Vec<PluginEvent> {
        // Called once after init to drain any startup events.
        // After that, all events flow through the unified channel (no polling).
        // No pending_events field anymore — events go through the unified channel.
        Vec::new()
    }

    fn collect_spectrum(&mut self) -> Vec<(ChannelId, Vec<(f32, f32)>)> {
        tracing::trace!(
            channels = self.channel_sinks.len(),
            "collecting spectrum data for all channels"
        );
        let mut results = Vec::new();
        for (&channel_id, sink_name) in &self.channel_sinks {
            let samples = spectrum::capture_monitor_samples(sink_name);
            if !samples.is_empty() {
                let bins = spectrum::samples_to_spectrum(&samples);
                if !bins.is_empty() {
                    tracing::trace!(
                        channel_id,
                        bins = bins.len(),
                        "spectrum data captured for channel"
                    );
                    results.push((channel_id, bins));
                }
            }
        }
        tracing::debug!(
            channels_with_data = results.len(),
            "spectrum collection complete"
        );
        results
    }

    fn cleanup(&mut self) -> Result<()> {
        let module_count = self.modules.module_count();
        tracing::info!(module_count = module_count, "PulseAudio plugin cleaning up");

        // Kill the pactl subscribe child process
        if let Some(mut child) = self.subscribe_process.take() {
            tracing::debug!("killing pactl subscribe process");
            let _ = child.kill();
            let _ = child.wait();
        }

        self.modules.unload_all(self.connection.as_mut());
        // Disconnect the PA verification connection
        if let Some(mut conn) = self.connection.take() {
            conn.disconnect();
        }
        tracing::info!(
            modules_unloaded = module_count,
            "PulseAudio plugin cleanup complete"
        );
        Ok(())
    }
}

/// Remove any orphaned osg_ null-sink modules from previous crashed sessions.
/// Called at startup before creating new sinks.
fn cleanup_orphaned_osg_modules() {
    let output = Command::new("pactl")
        .args(["list", "modules", "short"])
        .output();
    let Ok(output) = output else {
        tracing::warn!("failed to list modules for orphan cleanup");
        return;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut cleaned = 0u32;
    for line in text.lines() {
        // Format: "MODULE_ID\tmodule-null-sink\tsink_name=osg_..."
        if line.contains("osg_") {
            if let Some(id_str) = line.split_whitespace().next() {
                if let Ok(id) = id_str.parse::<u32>() {
                    let _ = Command::new("pactl")
                        .args(["unload-module", &id.to_string()])
                        .output();
                    cleaned += 1;
                }
            }
        }
    }
    if cleaned > 0 {
        tracing::info!(
            count = cleaned,
            "cleaned up orphaned OSG modules from previous session"
        );
    }
}
