//! PulseAudio plugin — the first audio backend for Open Sound Grid.
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
pub mod spectrum;

use std::collections::HashMap;
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc as std_mpsc;

use crate::effects::EffectsChain;
use crate::error::{OsgError, Result};
use crate::plugin::api::*;
use crate::plugin::{API_VERSION, AudioPlugin, PluginCapabilities, PluginInfo};

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
    /// Maps (channel_id) -> null-sink module ID for unloading on remove.
    channel_null_sink_modules: HashMap<u32, u32>,
    /// Maps (mix_id) -> PA sink name for the mix's null sink.
    mix_sinks: HashMap<u32, String>,
    /// Maps (mix_id) -> null-sink module ID for unloading on remove.
    mix_null_sink_modules: HashMap<u32, u32>,
    /// Maps (source, mix) -> loopback module id.
    loopback_modules: HashMap<(SourceId, MixId), u32>,
    /// Maps (source, mix) -> sink-input index for volume control.
    loopback_sink_inputs: HashMap<(SourceId, MixId), u32>,
    /// Maps mix_id -> (loopback module_id, output device_id) for mix-to-hardware output.
    mix_output_modules: HashMap<MixId, u32>,
    /// Per-channel effects chains. Keyed by ChannelId.
    effects_chains: HashMap<ChannelId, EffectsChain>,
    /// Loopback latency in milliseconds (from config).
    latency_ms: u32,
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
            channel_null_sink_modules: HashMap::new(),
            mix_sinks: HashMap::new(),
            mix_null_sink_modules: HashMap::new(),
            loopback_modules: HashMap::new(),
            loopback_sink_inputs: HashMap::new(),
            mix_output_modules: HashMap::new(),
            effects_chains: HashMap::new(),
            latency_ms: 20,
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
                tracing::warn!(
                    "build_snapshot: list_inputs returned empty (PA may be disconnected)"
                );
            }
            v
        };
        let hardware_outputs = {
            let v = DeviceEnumerator::list_outputs(self.connection.as_mut());
            if v.is_empty() {
                tracing::warn!(
                    "build_snapshot: list_outputs returned empty (PA may be disconnected)"
                );
            }
            v
        };
        let mut applications = match self.apps.list_applications(self.connection.as_mut()) {
            Ok(apps) => apps,
            Err(e) => {
                tracing::warn!(err = %e, "build_snapshot: list_applications failed — returning empty list");
                Vec::new()
            }
        };

        // Populate AudioApplication.channel from channel.apps
        for app in &mut applications {
            for channel in &self.channels {
                if channel.apps.contains(&app.stream_index) {
                    app.channel = Some(channel.id);
                    tracing::trace!(app_name = %app.name, channel_id = channel.id, "app routed to channel in snapshot");
                    break;
                }
            }
        }

        // Peak levels are read from the SharedPeak atomics via get_levels() —
        // lock-free and instant. read_peaks() (which spawned pactl subprocesses)
        // has been removed from this path; peaks are updated independently by
        // the PeakMonitor background thread and do not block state rebuilds.
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

            PluginCommand::ListHardwareInputs => Ok(PluginResponse::HardwareInputs(
                DeviceEnumerator::list_inputs(self.connection.as_mut()),
            )),

            PluginCommand::ListHardwareOutputs => Ok(PluginResponse::HardwareOutputs(
                DeviceEnumerator::list_outputs(self.connection.as_mut()),
            )),

            PluginCommand::ListApplications => {
                let apps = self.apps.list_applications(self.connection.as_mut())?;
                tracing::debug!(count = apps.len(), "listing applications");
                Ok(PluginResponse::Applications(apps))
            }

            PluginCommand::CreateChannel { name } => {
                // Skip if a channel with this name already exists
                if self.channels.iter().any(|c| c.name == name) {
                    tracing::debug!(name = %name, "channel already exists — skipping creation");
                    let existing_id = self.channels.iter().find(|c| c.name == name).unwrap().id;
                    return Ok(PluginResponse::ChannelCreated { id: existing_id });
                }
                let id = self.next_channel_id;
                self.next_channel_id += 1;

                let sink_name = Self::channel_sink_name(&name);
                let description = format!("OSG {name} Channel");

                match self.modules.create_null_sink(
                    self.connection.as_mut(),
                    &sink_name,
                    &description,
                ) {
                    Ok(null_sink_module_id) => {
                        tracing::info!(channel_name = %name, channel_id = id, sink_name = %sink_name, null_sink_module_id, "channel created");
                        self.peaks
                            .start_monitoring(&sink_name, SourceId::Channel(id));

                        // NOTE: We intentionally do NOT set the System channel as
                        // the default PA sink. The user's chosen OS output device
                        // must remain as their default. Apps are routed to channels
                        // explicitly via move-sink-input on assignment.

                        self.channel_sinks.insert(id, sink_name);
                        self.channel_null_sink_modules.insert(id, null_sink_module_id);
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
                    icon_path: None,
                    assigned_app_binaries: vec![],
                    muted: false,
                    effects,
                    master_volume: 1.0,
                });

                Ok(PluginResponse::ChannelCreated { id })
            }

            PluginCommand::RemoveChannel { id } => {
                tracing::info!(channel_id = id, "removing channel");
                self.channels.retain(|c| c.id != id);
                self.channel_sinks.remove(&id);
                self.peaks.stop_monitoring(&SourceId::Channel(id));

                // Remove all loopbacks involving this channel
                let source = SourceId::Channel(id);
                let keys_to_remove: Vec<_> = self
                    .loopback_modules
                    .keys()
                    .filter(|(src, _)| *src == source)
                    .cloned()
                    .collect();
                tracing::debug!(
                    channel_id = id,
                    loopbacks_to_remove = keys_to_remove.len(),
                    "cleaning up channel loopbacks"
                );
                for key in &keys_to_remove {
                    if let Some(module_id) = self.loopback_modules.remove(key) {
                        let _ = self
                            .modules
                            .unload_module(self.connection.as_mut(), module_id);
                    }
                    self.loopback_sink_inputs.remove(key);
                }

                self.routes.retain(|(src, _), _| *src != source);
                self.effects_chains.remove(&id);

                // Unload the channel's null-sink module (prevents PA resource leak)
                if let Some(null_sink_mod) = self.channel_null_sink_modules.remove(&id) {
                    tracing::debug!(channel_id = id, null_sink_mod, "unloading channel null-sink module");
                    let _ = self.modules.unload_module(self.connection.as_mut(), null_sink_mod);
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::RenameChannel { id, name } => {
                tracing::info!(channel_id = id, new_name = %name, "renaming channel");
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
                    ch.name = name;
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::CreateMix { name } => {
                // Skip if a mix with this name already exists (prevents config-load doubling)
                if self.mixes.iter().any(|m| m.name == name) {
                    tracing::debug!(name = %name, "mix already exists — skipping creation");
                    let existing_id = self.mixes.iter().find(|m| m.name == name).unwrap().id;
                    return Ok(PluginResponse::MixCreated { id: existing_id });
                }
                let id = self.next_mix_id;
                self.next_mix_id += 1;

                let sink_name = Self::mix_sink_name(&name);
                let description = format!("OSG {name} Mix");

                match self.modules.create_null_sink(
                    self.connection.as_mut(),
                    &sink_name,
                    &description,
                ) {
                    Ok(null_sink_module_id) => {
                        tracing::info!(mix_name = %name, mix_id = id, sink_name = %sink_name, null_sink_module_id, "mix created");
                        self.peaks.start_monitoring(&sink_name, SourceId::Mix(id));
                        self.mix_sinks.insert(id, sink_name);
                        self.mix_null_sink_modules.insert(id, null_sink_module_id);
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
                self.peaks.stop_monitoring(&SourceId::Mix(id));

                // Remove all loopbacks targeting this mix
                let keys_to_remove: Vec<_> = self
                    .loopback_modules
                    .keys()
                    .filter(|(_, mix)| *mix == id)
                    .cloned()
                    .collect();
                tracing::debug!(
                    mix_id = id,
                    loopbacks_to_remove = keys_to_remove.len(),
                    "cleaning up mix loopbacks"
                );
                for key in &keys_to_remove {
                    if let Some(module_id) = self.loopback_modules.remove(key) {
                        let _ = self
                            .modules
                            .unload_module(self.connection.as_mut(), module_id);
                    }
                    self.loopback_sink_inputs.remove(key);
                }

                self.routes.retain(|(_, mix), _| *mix != id);

                // Unload mix-to-hardware output loopback (prevents PA resource leak)
                if let Some(output_mod) = self.mix_output_modules.remove(&id) {
                    tracing::debug!(mix_id = id, output_mod, "unloading mix output loopback module");
                    let _ = self.modules.unload_module(self.connection.as_mut(), output_mod);
                }

                // Unload the mix's null-sink module
                if let Some(null_sink_mod) = self.mix_null_sink_modules.remove(&id) {
                    tracing::debug!(mix_id = id, null_sink_mod, "unloading mix null-sink module");
                    let _ = self.modules.unload_module(self.connection.as_mut(), null_sink_mod);
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::RenameMix { id, name } => {
                tracing::info!(mix_id = id, new_name = %name, "renaming mix");
                if let Some(mx) = self.mixes.iter_mut().find(|m| m.id == id) {
                    mx.name = name;
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteVolume {
                source,
                mix,
                volume,
            } => {
                let volume = volume.clamp(0.0, 1.0);
                tracing::debug!(source = ?source, mix = mix, volume = volume, "setting route volume");
                self.routes.entry((source, mix)).or_default().volume = volume;

                // Apply via PA if we have a sink-input for this route
                if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
                    tracing::debug!(
                        source = ?source, mix = mix, sink_input_idx, volume,
                        "applying route volume to PA sink-input"
                    );
                    if let Err(e) = self.modules.set_sink_input_volume(
                        self.connection.as_mut(),
                        sink_input_idx,
                        volume,
                    ) {
                        tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set route volume via PA");
                    }
                } else {
                    tracing::warn!(
                        source = ?source, mix = mix, volume,
                        loopback_count = self.loopback_sink_inputs.len(),
                        "SetRouteVolume: no sink-input found for route — volume change lost"
                    );
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteEnabled {
                source,
                mix,
                enabled,
            } => {
                tracing::debug!(source = ?source, mix = mix, enabled = enabled, "setting route enabled");

                if enabled {
                    // Resolve the source sink name for the monitor
                    let channel_id = match source {
                        SourceId::Channel(id) => id,
                        SourceId::Hardware(hw_id) => {
                            // Hardware input routing: find the PA source name from the
                            // current snapshot's hardware_inputs list
                            let hw_inputs = DeviceEnumerator::list_inputs(self.connection.as_mut());
                            let hw_source = hw_inputs
                                .iter()
                                .find(|h| h.id == hw_id)
                                .map(|h| h.device_id.clone());
                            if let Some(source_name) = hw_source {
                                let mix_sink = self
                                    .mix_sinks
                                    .get(&mix)
                                    .cloned()
                                    .ok_or_else(|| OsgError::MixNotFound(mix))?;
                                tracing::debug!(
                                    hw_source = %source_name,
                                    mix_sink = %mix_sink,
                                    "creating loopback for hardware input route"
                                );
                                let module_id = self.modules.create_loopback(
                                    self.connection.as_mut(),
                                    &source_name,
                                    &mix_sink,
                                    self.latency_ms,
                                )?;
                                self.loopback_modules.insert((source, mix), module_id);

                                // Discover sink-input for volume control (same as software channels)
                                match self.modules.find_loopback_sink_input(self.connection.as_mut(), module_id)? {
                                    Some(idx) => {
                                        tracing::debug!(module_id, sink_input_idx = idx, "found hardware loopback sink-input");
                                        self.loopback_sink_inputs.insert((source, mix), idx);
                                    }
                                    None => {
                                        tracing::warn!(module_id, "hardware loopback sink-input not found — volume control unavailable");
                                    }
                                }

                                self.routes.entry((source, mix)).or_default().enabled = true;
                                return Ok(PluginResponse::Ok);
                            } else {
                                tracing::warn!(hw_id, "hardware input not found for routing");
                                return Ok(PluginResponse::Ok);
                            }
                        }
                        SourceId::Mix(_) => {
                            tracing::warn!(source = ?source, "mix-as-source routing not supported");
                            return Ok(PluginResponse::Ok);
                        }
                    };

                    let channel_sink =
                        self.channel_sinks
                            .get(&channel_id)
                            .cloned()
                            .ok_or_else(|| {
                                tracing::error!(
                                    channel_id = channel_id,
                                    "channel sink not found for route enable"
                                );
                                OsgError::ChannelNotFound(channel_id)
                            })?;

                    let mix_sink = self.mix_sinks.get(&mix).cloned().ok_or_else(|| {
                        tracing::error!(mix_id = mix, "mix sink not found for route enable");
                        OsgError::MixNotFound(mix)
                    })?;

                    let source_monitor = format!("{channel_sink}.monitor");

                    // Teardown existing loopback if one already exists (prevents module leak
                    // when SetRouteEnabled is called again for an already-enabled route).
                    if let Some(old_module_id) = self.loopback_modules.remove(&(source, mix)) {
                        tracing::debug!(old_module_id, source = ?source, mix, "tearing down existing loopback before re-creation");
                        let _ = self.modules.unload_module(self.connection.as_mut(), old_module_id);
                        self.loopback_sink_inputs.remove(&(source, mix));
                    }

                    tracing::debug!(source_monitor = %source_monitor, mix_sink = %mix_sink, "creating loopback for route");

                    let module_id = self.modules.create_loopback(
                        self.connection.as_mut(),
                        &source_monitor,
                        &mix_sink,
                        self.latency_ms,
                    )?;
                    tracing::debug!(module_id = module_id, source = ?source, mix = mix, "loopback module created");
                    self.loopback_modules.insert((source, mix), module_id);

                    // find_loopback_sink_input has its own retry logic (3 attempts, 100ms each)
                    match self
                        .modules
                        .find_loopback_sink_input(self.connection.as_mut(), module_id)?
                    {
                        Some(idx) => {
                            tracing::debug!(
                                module_id,
                                sink_input_idx = idx,
                                "found loopback sink-input"
                            );
                            self.loopback_sink_inputs.insert((source, mix), idx);
                        }
                        None => {
                            tracing::warn!(
                                module_id,
                                "loopback sink-input not found — volume control unavailable for this route"
                            );
                        }
                    }

                    self.routes.entry((source, mix)).or_default().enabled = true;
                } else {
                    // Disable: tear down loopback
                    if let Some(module_id) = self.loopback_modules.remove(&(source, mix)) {
                        tracing::debug!(module_id = module_id, source = ?source, mix = mix, "unloading loopback module for route disable");
                        if let Err(e) = self
                            .modules
                            .unload_module(self.connection.as_mut(), module_id)
                        {
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
                    if let Err(e) = self.modules.set_sink_input_mute(
                        self.connection.as_mut(),
                        sink_input_idx,
                        muted,
                    ) {
                        tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set route mute via PA");
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::RouteApp { app, channel } => {
                tracing::debug!(app_id = app, channel_id = channel, "routing app to channel");
                let sink_name = self.channel_sinks.get(&channel).cloned().ok_or_else(|| {
                    tracing::error!(channel_id = channel, "channel not found for app routing");
                    OsgError::ChannelNotFound(channel)
                })?;

                if let Err(e) =
                    self.modules
                        .move_sink_input(self.connection.as_mut(), app, &sink_name)
                {
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
                tracing::debug!(app_id = app, "unrouting app — moving to default sink");
                // Move the app's stream back to the default PA sink
                if let Err(e) =
                    self.modules
                        .move_sink_input(self.connection.as_mut(), app, "@DEFAULT_SINK@")
                {
                    tracing::warn!(app_id = app, err = %e, "failed to move sink-input to default sink during unroute");
                }
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
                    tracing::debug!(
                        mix_id = mix,
                        old_module_id = old_module_id,
                        "unloading previous mix output loopback"
                    );
                    if let Err(e) = self
                        .modules
                        .unload_module(self.connection.as_mut(), old_module_id)
                    {
                        tracing::warn!(mix_id = mix, old_module_id = old_module_id, err = %e, "failed to unload previous mix output loopback");
                    }
                }

                // Create loopback from mix monitor to hardware output
                let source_monitor = format!("{mix_sink}.monitor");
                tracing::debug!(source_monitor = %source_monitor, hw_sink = %hw_device.device_id, "creating mix output loopback");

                let module_id = self.modules.create_loopback(
                    self.connection.as_mut(),
                    &source_monitor,
                    &hw_device.device_id,
                    self.latency_ms,
                )?;
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
                    let clamped = m.master_volume;
                    // Apply via PA: set volume on the mix null sink itself.
                    if let Some(sink_name) = self.mix_sinks.get(&mix).cloned() {
                        if let Some(conn) = self.connection.as_mut() {
                            if let Err(e) =
                                introspect::set_sink_volume_by_name_sync(conn, &sink_name, clamped)
                            {
                                tracing::warn!(mix_id = mix, err = %e, "set_sink_volume_by_name_sync failed");
                            } else {
                                tracing::debug!(mix_id = mix, volume = clamped, sink = %sink_name, "PA set-sink-volume applied via introspect");
                            }
                        } else {
                            let percent = (clamped * 100.0) as u32;
                            let output = Command::new("pactl")
                                .args(["set-sink-volume", &sink_name, &format!("{percent}%")])
                                .output();
                            match output {
                                Ok(o) if o.status.success() => {
                                    tracing::debug!(mix_id = mix, percent, sink = %sink_name, "PA set-sink-volume applied via pactl (fallback)");
                                }
                                Ok(o) => {
                                    tracing::warn!(mix_id = mix, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-volume failed");
                                }
                                Err(e) => {
                                    tracing::warn!(mix_id = mix, err = %e, "PA set-sink-volume command error");
                                }
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
                    // Apply via PA: mute the mix null sink.
                    if let Some(sink_name) = self.mix_sinks.get(&mix).cloned() {
                        if let Some(conn) = self.connection.as_mut() {
                            if let Err(e) =
                                introspect::set_sink_mute_by_name_sync(conn, &sink_name, muted)
                            {
                                tracing::warn!(mix_id = mix, err = %e, "set_sink_mute_by_name_sync failed");
                            } else {
                                tracing::debug!(mix_id = mix, muted, sink = %sink_name, "PA set-sink-mute applied to mix via introspect");
                            }
                        } else {
                            let mute_val = if muted { "1" } else { "0" };
                            let output = Command::new("pactl")
                                .args(["set-sink-mute", &sink_name, mute_val])
                                .output();
                            match output {
                                Ok(o) if o.status.success() => {
                                    tracing::debug!(mix_id = mix, muted, sink = %sink_name, "PA set-sink-mute applied to mix via pactl (fallback)");
                                }
                                Ok(o) => {
                                    tracing::warn!(mix_id = mix, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-mute failed on mix");
                                }
                                Err(e) => {
                                    tracing::warn!(mix_id = mix, err = %e, "PA set-sink-mute command error on mix");
                                }
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
                // Update in-memory state AND apply to loopback sink-inputs
                let sink_input_keys: Vec<(SourceId, MixId)> = self
                    .loopback_sink_inputs
                    .keys()
                    .filter(|(src, _)| *src == source)
                    .cloned()
                    .collect();
                for key in &sink_input_keys {
                    if let Some(&idx) = self.loopback_sink_inputs.get(key) {
                        if let Err(e) = self.modules.set_sink_input_mute(
                            self.connection.as_mut(),
                            idx,
                            muted,
                        ) {
                            tracing::warn!(source = ?source, sink_input = idx, err = %e, "failed to set sink-input mute via PA");
                        }
                    }
                }
                for ((src, _), route) in &mut self.routes {
                    if *src == source {
                        route.muted = muted;
                    }
                }
                // Apply via PA: mute the channel's null sink directly.
                if let SourceId::Channel(id) = source {
                    if let Some(sink_name) = self.channel_sinks.get(&id).cloned() {
                        if let Some(conn) = self.connection.as_mut() {
                            if let Err(e) =
                                introspect::set_sink_mute_by_name_sync(conn, &sink_name, muted)
                            {
                                tracing::warn!(source = ?source, err = %e, "set_sink_mute_by_name_sync failed for channel");
                            } else {
                                tracing::debug!(source = ?source, muted, sink = %sink_name, "PA set-sink-mute applied via introspect");
                            }
                        } else {
                            let mute_val = if muted { "1" } else { "0" };
                            let output = Command::new("pactl")
                                .args(["set-sink-mute", &sink_name, mute_val])
                                .output();
                            match output {
                                Ok(o) if o.status.success() => {
                                    tracing::debug!(source = ?source, muted, sink = %sink_name, "PA set-sink-mute applied via pactl (fallback)");
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
                    tracing::warn!(
                        channel_id = channel,
                        "SetEffectsParams: no effects chain found for channel"
                    );
                }
                // Sync params into ChannelInfo so snapshots reflect current state
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects = params;
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetEffectsEnabled { channel, enabled } => {
                tracing::debug!(
                    channel_id = channel,
                    enabled,
                    "setting effects enabled for channel"
                );
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_enabled(enabled);
                } else {
                    tracing::warn!(
                        channel_id = channel,
                        "SetEffectsEnabled: no effects chain found for channel"
                    );
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

    fn collect_spectrum(&mut self) -> Vec<(crate::plugin::api::ChannelId, Vec<(f32, f32)>)> {
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
