//! PulseAudio plugin — the first audio backend for OpenSoundGrid.
//!
//! Uses null sinks for channels, null sinks for mixes,
//! and module-loopback to connect them. Volume control
//! is done via sink-input volume on the loopback instances.
//!
//! Architecture:
//! - Each software channel → null sink (apps route here via move-sink-input)
//! - Each output mix → null sink (OBS/external apps capture from here)
//! - Each (channel, mix) pair → module-loopback connecting them
//! - Volume control → set-sink-input-volume on the loopback's sink-input

use std::collections::HashMap;

use crate::error::{OsgError, Result};
use crate::plugin::api::*;
use crate::plugin::{
    AudioPlugin, PluginCapabilities, PluginInfo, API_VERSION,
};

pub struct PulseAudioPlugin {
    connected: bool,
    next_channel_id: u32,
    next_mix_id: u32,
    channels: Vec<ChannelInfo>,
    mixes: Vec<MixInfo>,
    routes: HashMap<(SourceId, MixId), RouteState>,
    pending_events: Vec<PluginEvent>,
}

impl PulseAudioPlugin {
    pub fn new() -> Self {
        Self {
            connected: false,
            next_channel_id: 1,
            next_mix_id: 1,
            channels: Vec::new(),
            mixes: Vec::new(),
            routes: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    fn build_snapshot(&self) -> MixerSnapshot {
        MixerSnapshot {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs: vec![], // TODO: query PA
            hardware_outputs: vec![], // TODO: query PA
            applications: vec![], // TODO: query PA
            peak_levels: HashMap::new(),
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
            can_apply_effects: false, // v0.2
            can_lock_devices: false,  // v0.3
            max_channels: Some(8),
            max_mixes: Some(5),
        }
    }

    fn init(&mut self) -> Result<()> {
        // TODO: Connect to PulseAudio server via libpulse-binding
        // - Create threaded mainloop
        // - Connect context
        // - Subscribe to sink, sink-input, source events
        // - Enumerate existing sinks and sources
        tracing::info!("PulseAudio plugin initialized (stub)");
        self.connected = true;
        Ok(())
    }

    fn handle_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        match cmd {
            PluginCommand::GetState => {
                Ok(PluginResponse::State(self.build_snapshot()))
            }

            PluginCommand::ListHardwareInputs => {
                // TODO: pactl list sources, filter out monitors
                Ok(PluginResponse::HardwareInputs(vec![]))
            }

            PluginCommand::ListHardwareOutputs => {
                // TODO: pactl list sinks, filter out virtual
                Ok(PluginResponse::HardwareOutputs(vec![]))
            }

            PluginCommand::ListApplications => {
                // TODO: pactl list sink-inputs with application.name
                Ok(PluginResponse::Applications(vec![]))
            }

            PluginCommand::CreateChannel { name } => {
                let id = self.next_channel_id;
                self.next_channel_id += 1;

                // TODO: pactl load-module module-null-sink sink_name={name}_Apps
                tracing::info!("Created channel '{}' (id={})", name, id);

                self.channels.push(ChannelInfo {
                    id,
                    name,
                    apps: vec![],
                    muted: false,
                });

                Ok(PluginResponse::ChannelCreated { id })
            }

            PluginCommand::RemoveChannel { id } => {
                // TODO: unload null sink + all associated loopbacks
                self.channels.retain(|c| c.id != id);
                self.routes.retain(|(src, _), _| *src != SourceId::Channel(id));
                Ok(PluginResponse::Ok)
            }

            PluginCommand::CreateMix { name } => {
                let id = self.next_mix_id;
                self.next_mix_id += 1;

                // TODO: pactl load-module module-null-sink sink_name={name}_Mix
                // TODO: create loopbacks from each channel to this mix
                tracing::info!("Created mix '{}' (id={})", name, id);

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
                self.routes.retain(|(_, mix), _| *mix != id);
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteVolume { source, mix, volume } => {
                let volume = volume.clamp(0.0, 1.0);
                // TODO: pactl set-sink-input-volume <loopback_idx> <percent>%
                self.routes.entry((source, mix)).or_default().volume = volume;
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteEnabled { source, mix, enabled } => {
                // TODO: load/unload loopback module
                self.routes.entry((source, mix)).or_default().enabled = enabled;
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteMuted { source, mix, muted } => {
                // TODO: pactl set-sink-input-mute
                self.routes.entry((source, mix)).or_default().muted = muted;
                Ok(PluginResponse::Ok)
            }

            PluginCommand::RouteApp { app, channel } => {
                // TODO: pactl move-sink-input <app_stream_idx> <channel_sink>
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    if !ch.apps.contains(&app) {
                        ch.apps.push(app);
                    }
                    Ok(PluginResponse::Ok)
                } else {
                    Err(OsgError::ChannelNotFound(channel))
                }
            }

            PluginCommand::UnrouteApp { app } => {
                for ch in &mut self.channels {
                    ch.apps.retain(|&a| a != app);
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixOutput { mix, output } => {
                // TODO: create loopback from mix monitor → output device
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.output = Some(output);
                    Ok(PluginResponse::Ok)
                } else {
                    Err(OsgError::MixNotFound(mix))
                }
            }

            PluginCommand::SetMixMasterVolume { mix, volume } => {
                // TODO: pactl set-sink-volume <mix_sink> <percent>%
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
                // Mute across all mixes
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
        // TODO: check PA event subscription for device/app changes
        // TODO: read peak levels from PA monitor sources
        self.pending_events.drain(..).collect()
    }

    fn cleanup(&mut self) -> Result<()> {
        // TODO: unload all modules, disconnect from PA
        tracing::info!("PulseAudio plugin cleanup (stub)");
        self.connected = false;
        Ok(())
    }
}
