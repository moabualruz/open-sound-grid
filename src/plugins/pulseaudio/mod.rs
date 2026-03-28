//! PulseAudio plugin — the first audio backend for OpenSoundGrid.
//!
//! Uses null sinks for channels, null sinks for mixes,
//! and module-loopback to connect them. Volume control
//! is done via sink-input volume on the loopback instances.
//!
//! Architecture:
//! - Each software channel -> null sink (apps route here via move-sink-input)
//! - Each output mix -> null sink (OBS/external apps capture from here)
//! - Each (channel, mix) pair -> module-loopback connecting them
//! - Volume control -> set-sink-input-volume on the loopback's sink-input

mod apps;
mod connection;
mod devices;
mod introspect;
mod modules;
mod peaks;

use std::collections::HashMap;
use std::io::BufRead;
use std::process::{Child, Stdio};
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::Duration;

use crate::effects::EffectsChain;
use crate::error::{OsgError, Result};
use crate::plugin::api::*;
use crate::plugin::{AudioPlugin, PluginCapabilities, PluginInfo, API_VERSION};

use self::apps::AppDetector;
use self::connection::PulseConnection;
use self::devices::DeviceEnumerator;
use self::modules::ModuleManager;
use self::peaks::PeakMonitor;

pub struct PulseAudioPlugin {
    connection: Option<PulseConnection>,
    modules: ModuleManager,
    apps: AppDetector,
    peaks: PeakMonitor,
    next_channel_id: u32,
    next_mix_id: u32,
    channels: Vec<ChannelInfo>,
    mixes: Vec<MixInfo>,
    routes: HashMap<(SourceId, MixId), RouteState>,
    /// Maps (channel_id) -> PA sink name for the channel's null sink.
    channel_sinks: HashMap<u32, String>,
    /// Maps (mix_id) -> PA sink name for the mix's null sink.
    mix_sinks: HashMap<u32, String>,
    /// Maps (source, mix) -> loopback module id.
    loopback_modules: HashMap<(SourceId, MixId), u32>,
    /// Maps (source, mix) -> sink-input index for volume control.
    loopback_sink_inputs: HashMap<(SourceId, MixId), u32>,
    /// Maps mix_id -> (loopback module_id, output device_id) for mix-to-hardware output.
    mix_output_modules: HashMap<MixId, u32>,
    /// Per-channel effects chains. Keyed by ChannelId.
    effects_chains: HashMap<ChannelId, EffectsChain>,
    /// `pactl subscribe` child process for PA event notifications.
    subscribe_process: Option<Child>,
    /// Sender for pushing PA subscribe events into the plugin thread's unified channel.
    /// Set via `set_event_sender()` by the plugin manager after init.
    unified_tx: Option<std_mpsc::Sender<crate::plugin::PluginThreadMsg>>,
}

// SAFETY: PulseAudioPlugin is moved into the plugin thread and only accessed there.
// The inner PulseConnection contains Rc<RefCell<>> which is !Send, but since we
// guarantee single-thread access this is safe.
unsafe impl Send for PulseAudioPlugin {}

impl PulseAudioPlugin {
    pub fn new() -> Self {
        tracing::debug!("creating PulseAudioPlugin instance");
        Self {
            connection: None,
            modules: ModuleManager::new(),
            apps: AppDetector::new(),
            peaks: PeakMonitor::new(),
            next_channel_id: 1,
            next_mix_id: 1,
            channels: Vec::new(),
            mixes: Vec::new(),
            routes: HashMap::new(),
            channel_sinks: HashMap::new(),
            mix_sinks: HashMap::new(),
            loopback_modules: HashMap::new(),
            loopback_sink_inputs: HashMap::new(),
            mix_output_modules: HashMap::new(),
            effects_chains: HashMap::new(),
            subscribe_process: None,
            unified_tx: None,
        }
    }

    fn build_snapshot(&mut self) -> MixerSnapshot {
        tracing::debug!(
            channels = self.channels.len(),
            mixes = self.mixes.len(),
            routes = self.routes.len(),
            "building mixer snapshot"
        );
        let hardware_inputs = {
            let v = DeviceEnumerator::list_inputs(self.connection.as_mut());
            if v.is_empty() {
                tracing::warn!("build_snapshot: list_inputs returned empty (PA may be disconnected)");
            }
            v
        };
        let hardware_outputs = {
            let v = DeviceEnumerator::list_outputs(self.connection.as_mut());
            if v.is_empty() {
                tracing::warn!("build_snapshot: list_outputs returned empty (PA may be disconnected)");
            }
            v
        };
        let applications = match self.apps.list_applications(self.connection.as_mut()) {
            Ok(apps) => apps,
            Err(e) => {
                tracing::warn!(err = %e, "build_snapshot: list_applications failed — returning empty list");
                Vec::new()
            }
        };

        // Refresh peak levels for all known channel sinks before snapshotting.
        // NOTE: These levels reflect the sink's configured volume, not the actual
        // signal amplitude. True peak monitoring via PA_STREAM_PEAK_DETECT streams
        // is a future improvement (requires unsafe callback wiring with libpulse).
        // channel_sinks is borrowed from self, so collect to avoid borrow conflict.
        let sink_pairs: Vec<(u32, String)> = self
            .channel_sinks
            .iter()
            .map(|(&id, name)| (id, name.clone()))
            .collect();
        for (id, sink_name) in &sink_pairs {
            tracing::trace!(channel_id = id, sink = %sink_name, "refreshing channel peak level");
            self.peaks.update_level(sink_name, SourceId::Channel(*id));
        }

        // Refresh peak levels for all known mix sinks.
        // Stored under SourceId::Mix so the UI can display VU meters in mix headers.
        let mix_pairs: Vec<(u32, String)> = self
            .mix_sinks
            .iter()
            .map(|(&id, name)| (id, name.clone()))
            .collect();
        for (id, sink_name) in &mix_pairs {
            tracing::trace!(mix_id = id, sink = %sink_name, "refreshing mix peak level");
            self.peaks.update_level(sink_name, SourceId::Mix(*id));
        }

        MixerSnapshot {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs,
            hardware_outputs,
            applications,
            peak_levels: self.peaks.get_levels(),
        }
    }

    /// Get the PA sink name for a channel.
    fn channel_sink_name(name: &str) -> String {
        format!("osg_{}_ch", name.replace(' ', "_"))
    }

    /// Get the PA sink name for a mix.
    fn mix_sink_name(name: &str) -> String {
        format!("osg_{}_mix", name.replace(' ', "_"))
    }

    /// Inner dispatch: execute a plugin command and return the result.
    /// Called by `handle_command` which adds the error-level tracing wrapper.
    fn dispatch_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        match cmd {
            PluginCommand::GetState => {
                tracing::debug!("building mixer snapshot");
                Ok(PluginResponse::State(self.build_snapshot()))
            }

            PluginCommand::ListHardwareInputs => {
                Ok(PluginResponse::HardwareInputs(DeviceEnumerator::list_inputs(self.connection.as_mut())))
            }

            PluginCommand::ListHardwareOutputs => {
                Ok(PluginResponse::HardwareOutputs(DeviceEnumerator::list_outputs(self.connection.as_mut())))
            }

            PluginCommand::ListApplications => {
                let apps = self.apps.list_applications(self.connection.as_mut())?;
                tracing::debug!(count = apps.len(), "listing applications");
                Ok(PluginResponse::Applications(apps))
            }

            PluginCommand::CreateChannel { name } => {
                let id = self.next_channel_id;
                self.next_channel_id += 1;

                let sink_name = Self::channel_sink_name(&name);
                let description = format!("OSG {name} Channel");

                match self.modules.create_null_sink(&sink_name, &description) {
                    Ok(_module_id) => {
                        tracing::info!(channel_name = %name, channel_id = id, sink_name = %sink_name, "channel created");
                        self.channel_sinks.insert(id, sink_name);
                    }
                    Err(e) => {
                        tracing::error!(channel_name = %name, err = %e, "failed to create null sink for channel");
                        return Err(e);
                    }
                }

                let effects = crate::effects::EffectsParams::default();
                self.effects_chains.insert(id, EffectsChain::new());
                self.channels.push(ChannelInfo {
                    id,
                    name,
                    apps: vec![],
                    muted: false,
                    effects,
                });

                Ok(PluginResponse::ChannelCreated { id })
            }

            PluginCommand::RemoveChannel { id } => {
                tracing::info!(channel_id = id, "removing channel");
                self.channels.retain(|c| c.id != id);
                self.channel_sinks.remove(&id);

                // Remove all loopbacks involving this channel
                let source = SourceId::Channel(id);
                let keys_to_remove: Vec<_> = self
                    .loopback_modules
                    .keys()
                    .filter(|(src, _)| *src == source)
                    .cloned()
                    .collect();
                tracing::debug!(channel_id = id, loopbacks_to_remove = keys_to_remove.len(), "cleaning up channel loopbacks");
                for key in &keys_to_remove {
                    if let Some(module_id) = self.loopback_modules.remove(key) {
                        let _ = self.modules.unload_module(module_id);
                    }
                    self.loopback_sink_inputs.remove(key);
                }

                self.routes.retain(|(src, _), _| *src != source);
                self.effects_chains.remove(&id);
                tracing::debug!(channel_id = id, "removed effects chain for channel");
                Ok(PluginResponse::Ok)
            }

            PluginCommand::CreateMix { name } => {
                let id = self.next_mix_id;
                self.next_mix_id += 1;

                let sink_name = Self::mix_sink_name(&name);
                let description = format!("OSG {name} Mix");

                match self.modules.create_null_sink(&sink_name, &description) {
                    Ok(_module_id) => {
                        tracing::info!(mix_name = %name, mix_id = id, sink_name = %sink_name, "mix created");
                        self.mix_sinks.insert(id, sink_name);
                    }
                    Err(e) => {
                        tracing::error!(mix_name = %name, err = %e, "failed to create null sink for mix");
                        return Err(e);
                    }
                }

                self.mixes.push(MixInfo {
                    id,
                    name,
                    output: None,
                    master_volume: 1.0,
                    muted: false,
                });

                Ok(PluginResponse::MixCreated { id })
            }

            PluginCommand::RemoveMix { id } => {
                tracing::info!(mix_id = id, "removing mix");
                self.mixes.retain(|m| m.id != id);
                self.mix_sinks.remove(&id);

                // Remove all loopbacks targeting this mix
                let keys_to_remove: Vec<_> = self
                    .loopback_modules
                    .keys()
                    .filter(|(_, mix)| *mix == id)
                    .cloned()
                    .collect();
                tracing::debug!(mix_id = id, loopbacks_to_remove = keys_to_remove.len(), "cleaning up mix loopbacks");
                for key in &keys_to_remove {
                    if let Some(module_id) = self.loopback_modules.remove(key) {
                        let _ = self.modules.unload_module(module_id);
                    }
                    self.loopback_sink_inputs.remove(key);
                }

                self.routes.retain(|(_, mix), _| *mix != id);
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteVolume { source, mix, volume } => {
                let volume = volume.clamp(0.0, 1.0);
                tracing::debug!(source = ?source, mix = mix, volume = volume, "setting route volume");
                self.routes.entry((source, mix)).or_default().volume = volume;

                // Apply via PA if we have a sink-input for this route
                if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
                    if let Err(e) = self.modules.set_sink_input_volume(sink_input_idx, volume) {
                        tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set route volume via PA");
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteEnabled { source, mix, enabled } => {
                tracing::debug!(source = ?source, mix = mix, enabled = enabled, "setting route enabled");

                if enabled {
                    // Resolve the source sink name for the monitor
                    let channel_id = match source {
                        SourceId::Channel(id) => id,
                        SourceId::Hardware(_) => {
                            tracing::warn!(source = ?source, "hardware source routing not yet supported");
                            self.routes.entry((source, mix)).or_default().enabled = true;
                            return Ok(PluginResponse::Ok);
                        }
                        SourceId::Mix(_) => {
                            tracing::warn!(source = ?source, "mix-as-source routing not supported");
                            return Ok(PluginResponse::Ok);
                        }
                    };

                    let channel_sink = self.channel_sinks.get(&channel_id).cloned().ok_or_else(|| {
                        tracing::error!(channel_id = channel_id, "channel sink not found for route enable");
                        OsgError::ChannelNotFound(channel_id)
                    })?;

                    let mix_sink = self.mix_sinks.get(&mix).cloned().ok_or_else(|| {
                        tracing::error!(mix_id = mix, "mix sink not found for route enable");
                        OsgError::MixNotFound(mix)
                    })?;

                    let source_monitor = format!("{channel_sink}.monitor");
                    tracing::debug!(source_monitor = %source_monitor, mix_sink = %mix_sink, "creating loopback for route");

                    let module_id = self.modules.create_loopback(&source_monitor, &mix_sink, 20)?;
                    tracing::debug!(module_id = module_id, source = ?source, mix = mix, "loopback module created");
                    self.loopback_modules.insert((source, mix), module_id);

                    // Small delay for PipeWire/PA to register the sink-input
                    tracing::debug!(module_id = module_id, "waiting 50ms for PA to register sink-input");
                    thread::sleep(Duration::from_millis(50));

                    match self.modules.find_loopback_sink_input(module_id)? {
                        Some(idx) => {
                            tracing::debug!(module_id, sink_input_idx = idx, "found loopback sink-input");
                            self.loopback_sink_inputs.insert((source, mix), idx);
                        }
                        None => {
                            tracing::warn!(module_id, "loopback sink-input not found — volume control unavailable for this route");
                        }
                    }

                    self.routes.entry((source, mix)).or_default().enabled = true;
                } else {
                    // Disable: tear down loopback
                    if let Some(module_id) = self.loopback_modules.remove(&(source, mix)) {
                        tracing::debug!(module_id = module_id, source = ?source, mix = mix, "unloading loopback module for route disable");
                        if let Err(e) = self.modules.unload_module(module_id) {
                            tracing::warn!(module_id = module_id, err = %e, "failed to unload loopback module");
                        }
                    }
                    self.loopback_sink_inputs.remove(&(source, mix));
                    self.routes.remove(&(source, mix));
                    tracing::debug!(source = ?source, mix = mix, "route disabled and removed");
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteMuted { source, mix, muted } => {
                tracing::debug!(source = ?source, mix = mix, muted = muted, "setting route muted");
                self.routes.entry((source, mix)).or_default().muted = muted;

                if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
                    if let Err(e) = self.modules.set_sink_input_mute(sink_input_idx, muted) {
                        tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set route mute via PA");
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::RouteApp { app, channel } => {
                tracing::debug!(app_id = app, channel_id = channel, "routing app to channel");
                let sink_name = self
                    .channel_sinks
                    .get(&channel)
                    .cloned()
                    .ok_or_else(|| {
                        tracing::error!(channel_id = channel, "channel not found for app routing");
                        OsgError::ChannelNotFound(channel)
                    })?;

                if let Err(e) = self.modules.move_sink_input(app, &sink_name) {
                    tracing::warn!(app_id = app, sink_name = %sink_name, err = %e, "failed to move sink-input for app routing");
                }

                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    if !ch.apps.contains(&app) {
                        ch.apps.push(app);
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::UnrouteApp { app } => {
                tracing::debug!(app_id = app, "unrouting app from all channels");
                for ch in &mut self.channels {
                    ch.apps.retain(|&a| a != app);
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixOutput { mix, output } => {
                tracing::debug!(mix_id = mix, output = output, "setting mix output");

                let mix_sink = self.mix_sinks.get(&mix).cloned().ok_or_else(|| {
                    tracing::error!(mix_id = mix, "mix sink not found for SetMixOutput");
                    OsgError::MixNotFound(mix)
                })?;

                // Find the hardware output device_id by OutputId
                let hw_outputs = DeviceEnumerator::list_outputs(self.connection.as_mut());
                let hw_device = hw_outputs.iter().find(|o| o.id == output).ok_or_else(|| {
                    tracing::error!(output_id = output, "hardware output not found");
                    OsgError::OutputNotFound(format!("output id {output}"))
                })?;

                // Tear down previous output loopback if any
                if let Some(old_module_id) = self.mix_output_modules.remove(&mix) {
                    tracing::debug!(mix_id = mix, old_module_id = old_module_id, "unloading previous mix output loopback");
                    if let Err(e) = self.modules.unload_module(old_module_id) {
                        tracing::warn!(mix_id = mix, old_module_id = old_module_id, err = %e, "failed to unload previous mix output loopback");
                    }
                }

                // Create loopback from mix monitor to hardware output
                let source_monitor = format!("{mix_sink}.monitor");
                tracing::debug!(source_monitor = %source_monitor, hw_sink = %hw_device.device_id, "creating mix output loopback");

                let module_id = self.modules.create_loopback(&source_monitor, &hw_device.device_id, 20)?;
                tracing::debug!(mix_id = mix, module_id = module_id, hw_sink = %hw_device.device_id, "mix output loopback created");
                self.mix_output_modules.insert(mix, module_id);

                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.output = Some(output);
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixMasterVolume { mix, volume } => {
                tracing::debug!(mix_id = mix, volume = volume, "setting mix master volume");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.master_volume = volume.clamp(0.0, 1.0);
                    // Apply via PA: set volume on the mix null sink itself
                    if let Some(sink_name) = self.mix_sinks.get(&mix) {
                        let percent = (m.master_volume * 100.0) as u32;
                        let output = std::process::Command::new("pactl")
                            .args(["set-sink-volume", sink_name, &format!("{percent}%")])
                            .output();
                        match output {
                            Ok(o) if o.status.success() => {
                                tracing::debug!(mix_id = mix, percent, sink = %sink_name, "PA set-sink-volume applied");
                            }
                            Ok(o) => {
                                tracing::warn!(mix_id = mix, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-volume failed");
                            }
                            Err(e) => {
                                tracing::warn!(mix_id = mix, err = %e, "PA set-sink-volume command error");
                            }
                        }
                    }
                    Ok(PluginResponse::Ok)
                } else {
                    tracing::error!(mix_id = mix, "mix not found for SetMixMasterVolume");
                    Err(OsgError::MixNotFound(mix))
                }
            }

            PluginCommand::SetMixMuted { mix, muted } => {
                tracing::debug!(mix_id = mix, muted = muted, "setting mix muted");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.muted = muted;
                    // Apply via PA: mute the mix null sink
                    if let Some(sink_name) = self.mix_sinks.get(&mix) {
                        let mute_val = if muted { "1" } else { "0" };
                        let output = std::process::Command::new("pactl")
                            .args(["set-sink-mute", sink_name, mute_val])
                            .output();
                        match output {
                            Ok(o) if o.status.success() => {
                                tracing::debug!(mix_id = mix, muted, sink = %sink_name, "PA set-sink-mute applied to mix");
                            }
                            Ok(o) => {
                                tracing::warn!(mix_id = mix, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-mute failed on mix");
                            }
                            Err(e) => {
                                tracing::warn!(mix_id = mix, err = %e, "PA set-sink-mute command error on mix");
                            }
                        }
                    }
                    Ok(PluginResponse::Ok)
                } else {
                    tracing::error!(mix_id = mix, "mix not found for SetMixMuted");
                    Err(OsgError::MixNotFound(mix))
                }
            }

            PluginCommand::SetSourceMuted { source, muted } => {
                tracing::debug!(source = ?source, muted = muted, "setting source muted across all routes");
                // Update in-memory state
                for ((src, _), route) in &mut self.routes {
                    if *src == source {
                        route.muted = muted;
                    }
                }
                // Apply via PA: mute the channel's null sink directly
                if let SourceId::Channel(id) = source {
                    if let Some(sink_name) = self.channel_sinks.get(&id) {
                        let mute_val = if muted { "1" } else { "0" };
                        let output = std::process::Command::new("pactl")
                            .args(["set-sink-mute", sink_name, mute_val])
                            .output();
                        match output {
                            Ok(o) if o.status.success() => {
                                tracing::debug!(source = ?source, muted, sink = %sink_name, "PA set-sink-mute applied");
                            }
                            Ok(o) => {
                                tracing::warn!(source = ?source, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-mute failed");
                            }
                            Err(e) => {
                                tracing::warn!(source = ?source, err = %e, "PA set-sink-mute command error");
                            }
                        }
                    }
                }
                // Also update channel muted state
                if let SourceId::Channel(id) = source {
                    if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
                        ch.muted = muted;
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetEffectsParams { channel, params } => {
                tracing::debug!(
                    channel_id = channel,
                    enabled = params.enabled,
                    eq_freq = params.eq_freq_hz,
                    comp_threshold = params.comp_threshold_db,
                    "setting effects params for channel"
                );
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_params(params.clone());
                } else {
                    tracing::warn!(channel_id = channel, "SetEffectsParams: no effects chain found for channel");
                }
                // Sync params into ChannelInfo so snapshots reflect current state
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects = params;
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetEffectsEnabled { channel, enabled } => {
                tracing::debug!(channel_id = channel, enabled, "setting effects enabled for channel");
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_enabled(enabled);
                } else {
                    tracing::warn!(channel_id = channel, "SetEffectsEnabled: no effects chain found for channel");
                }
                // Sync into ChannelInfo
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects.enabled = enabled;
                }
                Ok(PluginResponse::Ok)
            }
        }
    }
}

impl Drop for PulseAudioPlugin {
    fn drop(&mut self) {
        if let Some(mut child) = self.subscribe_process.take() {
            tracing::debug!("dropping PulseAudioPlugin — killing subscribe process");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl AudioPlugin for PulseAudioPlugin {
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
        use crate::plugin::manager::PaSubscribeKind;
        use crate::plugin::PluginThreadMsg;

        tracing::debug!("setting PA event sender — spawning pactl subscribe");
        self.unified_tx = Some(tx.clone());

        // Spawn `pactl subscribe` for real-time PA event notifications
        match std::process::Command::new("pactl")
            .arg("subscribe")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
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
                            let kind = if line.contains("sink-input") {
                                Some(PaSubscribeKind::SinkInput)
                            } else if line.contains("'change' on sink")
                                || line.contains("'new' on sink")
                                || line.contains("'remove' on sink")
                            {
                                Some(PaSubscribeKind::Sink)
                            } else if line.contains("source") {
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

    fn cleanup(&mut self) -> Result<()> {
        let module_count = self.modules.module_count();
        tracing::info!(module_count = module_count, "PulseAudio plugin cleaning up");

        // Kill the pactl subscribe child process
        if let Some(mut child) = self.subscribe_process.take() {
            tracing::debug!("killing pactl subscribe process");
            let _ = child.kill();
            let _ = child.wait();
        }

        self.modules.unload_all();
        // Disconnect the PA verification connection
        if let Some(mut conn) = self.connection.take() {
            conn.disconnect();
        }
        tracing::info!(modules_unloaded = module_count, "PulseAudio plugin cleanup complete");
        Ok(())
    }
}
