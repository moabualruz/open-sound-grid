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
mod modules;
mod peaks;

use std::collections::HashMap;

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
    pending_events: Vec<PluginEvent>,
}

// SAFETY: PulseAudioPlugin is moved into the plugin thread and only accessed there.
// The inner PulseConnection contains Rc<RefCell<>> which is !Send, but since we
// guarantee single-thread access this is safe.
unsafe impl Send for PulseAudioPlugin {}

impl PulseAudioPlugin {
    pub fn new() -> Self {
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
            pending_events: Vec::new(),
        }
    }

    fn build_snapshot(&self) -> MixerSnapshot {
        MixerSnapshot {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs: DeviceEnumerator::list_inputs(),
            hardware_outputs: DeviceEnumerator::list_outputs(),
            applications: self.apps.list_applications().unwrap_or_default(),
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
        let conn = PulseConnection::connect()?;
        self.connection = Some(conn);
        tracing::info!("PulseAudio plugin initialized");
        Ok(())
    }

    fn handle_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        match cmd {
            PluginCommand::GetState => {
                Ok(PluginResponse::State(self.build_snapshot()))
            }

            PluginCommand::ListHardwareInputs => {
                Ok(PluginResponse::HardwareInputs(DeviceEnumerator::list_inputs()))
            }

            PluginCommand::ListHardwareOutputs => {
                Ok(PluginResponse::HardwareOutputs(DeviceEnumerator::list_outputs()))
            }

            PluginCommand::ListApplications => {
                let apps = self.apps.list_applications()?;
                Ok(PluginResponse::Applications(apps))
            }

            PluginCommand::CreateChannel { name } => {
                let id = self.next_channel_id;
                self.next_channel_id += 1;

                let sink_name = Self::channel_sink_name(&name);
                let description = format!("OSG {name} Channel");

                match self.modules.create_null_sink(&sink_name, &description) {
                    Ok(_module_id) => {
                        tracing::info!("Created channel '{name}' (id={id}, sink={sink_name})");
                        self.channel_sinks.insert(id, sink_name);
                    }
                    Err(e) => {
                        tracing::error!("Failed to create null sink for channel '{name}': {e}");
                        return Err(e);
                    }
                }

                self.channels.push(ChannelInfo {
                    id,
                    name,
                    apps: vec![],
                    muted: false,
                });

                Ok(PluginResponse::ChannelCreated { id })
            }

            PluginCommand::RemoveChannel { id } => {
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
                for key in &keys_to_remove {
                    if let Some(module_id) = self.loopback_modules.remove(key) {
                        let _ = self.modules.unload_module(module_id);
                    }
                    self.loopback_sink_inputs.remove(key);
                }

                self.routes.retain(|(src, _), _| *src != source);
                Ok(PluginResponse::Ok)
            }

            PluginCommand::CreateMix { name } => {
                let id = self.next_mix_id;
                self.next_mix_id += 1;

                let sink_name = Self::mix_sink_name(&name);
                let description = format!("OSG {name} Mix");

                match self.modules.create_null_sink(&sink_name, &description) {
                    Ok(_module_id) => {
                        tracing::info!("Created mix '{name}' (id={id}, sink={sink_name})");
                        self.mix_sinks.insert(id, sink_name);
                    }
                    Err(e) => {
                        tracing::error!("Failed to create null sink for mix '{name}': {e}");
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
                self.mixes.retain(|m| m.id != id);
                self.mix_sinks.remove(&id);

                // Remove all loopbacks targeting this mix
                let keys_to_remove: Vec<_> = self
                    .loopback_modules
                    .keys()
                    .filter(|(_, mix)| *mix == id)
                    .cloned()
                    .collect();
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
                self.routes.entry((source, mix)).or_default().volume = volume;

                // Apply via PA if we have a sink-input for this route
                if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
                    if let Err(e) = self.modules.set_sink_input_volume(sink_input_idx, volume) {
                        tracing::warn!("Failed to set volume: {e}");
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteEnabled { source, mix, enabled } => {
                self.routes.entry((source, mix)).or_default().enabled = enabled;
                // TODO: load/unload loopback module based on enabled state
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteMuted { source, mix, muted } => {
                self.routes.entry((source, mix)).or_default().muted = muted;

                if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
                    if let Err(e) = self.modules.set_sink_input_mute(sink_input_idx, muted) {
                        tracing::warn!("Failed to set mute: {e}");
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::RouteApp { app, channel } => {
                let sink_name = self
                    .channel_sinks
                    .get(&channel)
                    .cloned()
                    .ok_or(OsgError::ChannelNotFound(channel))?;

                if let Err(e) = self.modules.move_sink_input(app, &sink_name) {
                    tracing::warn!("Failed to move sink-input {app} -> {sink_name}: {e}");
                }

                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    if !ch.apps.contains(&app) {
                        ch.apps.push(app);
                    }
                }

                Ok(PluginResponse::Ok)
            }

            PluginCommand::UnrouteApp { app } => {
                for ch in &mut self.channels {
                    ch.apps.retain(|&a| a != app);
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixOutput { mix, output } => {
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.output = Some(output);
                    Ok(PluginResponse::Ok)
                } else {
                    Err(OsgError::MixNotFound(mix))
                }
            }

            PluginCommand::SetMixMasterVolume { mix, volume } => {
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.master_volume = volume.clamp(0.0, 1.0);
                    Ok(PluginResponse::Ok)
                } else {
                    Err(OsgError::MixNotFound(mix))
                }
            }

            PluginCommand::SetMixMuted { mix, muted } => {
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.muted = muted;
                    Ok(PluginResponse::Ok)
                } else {
                    Err(OsgError::MixNotFound(mix))
                }
            }

            PluginCommand::SetSourceMuted { source, muted } => {
                for ((src, _), route) in &mut self.routes {
                    if *src == source {
                        route.muted = muted;
                    }
                }
                Ok(PluginResponse::Ok)
            }
        }
    }

    fn poll_events(&mut self) -> Vec<PluginEvent> {
        // Update peak levels for all channel sinks
        for channel in &self.channels {
            if let Some(sink_name) = self.channel_sinks.get(&channel.id) {
                self.peaks.update_level(sink_name, SourceId::Channel(channel.id));
            }
        }

        let levels = self.peaks.get_levels();
        if !levels.is_empty() {
            self.pending_events.push(PluginEvent::PeakLevels(levels));
        }

        self.pending_events.drain(..).collect()
    }

    fn cleanup(&mut self) -> Result<()> {
        tracing::info!("PulseAudio plugin cleaning up");
        self.modules.unload_all();
        if let Some(mut conn) = self.connection.take() {
            conn.disconnect();
        }
        Ok(())
    }
}
