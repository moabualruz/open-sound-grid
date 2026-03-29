//! PipeWire plugin — native audio backend using PW nodes and links.
//!
//! Architecture:
//! - Each software channel → virtual null-audio-sink node
//! - Each output mix → virtual null-audio-sink node
//! - Each (channel, mix) route → direct PW link between ports
//! - Volume control → node property adjustment
//! - Peak monitoring → capture stream on monitor port
//!
//! Falls back to the PulseAudio plugin if PipeWire is not available.

#[cfg(feature = "pipewire-backend")]
mod connection;
#[cfg(feature = "pipewire-backend")]
mod nodes;

#[cfg(feature = "pipewire-backend")]
use std::collections::HashMap;

#[cfg(feature = "pipewire-backend")]
use crate::effects::EffectsChain;
#[cfg(feature = "pipewire-backend")]
use crate::error::Result;
#[cfg(feature = "pipewire-backend")]
use crate::plugin::api::*;
#[cfg(feature = "pipewire-backend")]
use crate::plugin::{API_VERSION, AudioPlugin, PluginCapabilities, PluginInfo};

#[cfg(feature = "pipewire-backend")]
use self::connection::PwConnection;
#[cfg(feature = "pipewire-backend")]
use self::nodes::PwNodeManager;

#[cfg(feature = "pipewire-backend")]
pub struct PipeWirePlugin {
    connection: Option<PwConnection>,
    nodes: PwNodeManager,
    next_channel_id: u32,
    next_mix_id: u32,
    channels: Vec<ChannelInfo>,
    mixes: Vec<MixInfo>,
    routes: HashMap<(SourceId, MixId), RouteState>,
    effects_chains: HashMap<ChannelId, EffectsChain>,
    unified_tx: Option<std::sync::mpsc::Sender<crate::plugin::PluginThreadMsg>>,
}

// SAFETY: Same pattern as PulseAudioPlugin — plugin is moved into the plugin
// thread and only accessed there. PipeWire types use raw pointers (!Send).
#[cfg(feature = "pipewire-backend")]
unsafe impl Send for PipeWirePlugin {}

#[cfg(feature = "pipewire-backend")]
impl PipeWirePlugin {
    pub fn new() -> Self {
        tracing::debug!("creating new PipeWirePlugin");
        Self {
            connection: None,
            nodes: PwNodeManager::new(),
            next_channel_id: 1,
            next_mix_id: 1,
            channels: Vec::new(),
            mixes: Vec::new(),
            routes: HashMap::new(),
            effects_chains: HashMap::new(),
            unified_tx: None,
        }
    }

    fn build_snapshot(&self) -> MixerSnapshot {
        tracing::debug!(
            channels = self.channels.len(),
            mixes = self.mixes.len(),
            routes = self.routes.len(),
            "building PW mixer snapshot"
        );
        MixerSnapshot {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs: vec![],  // TODO: enumerate PW sources
            hardware_outputs: vec![], // TODO: enumerate PW sinks
            applications: vec![],     // TODO: enumerate PW streams
            peak_levels: HashMap::new(),
        }
    }
}

#[cfg(feature = "pipewire-backend")]
impl AudioPlugin for PipeWirePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pipewire",
            name: "PipeWire",
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
            can_apply_effects: false, // two-stream DSP is future
            can_lock_devices: false,
            max_channels: None, // PW has no practical limit
            max_mixes: None,
        }
    }

    fn init(&mut self) -> Result<()> {
        tracing::debug!("initializing PipeWire plugin");
        let conn = PwConnection::connect()?;
        tracing::info!(
            plugin_id = "pipewire",
            version = "0.1.0",
            "PipeWire plugin initialized"
        );
        self.connection = Some(conn);
        Ok(())
    }

    fn handle_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        tracing::debug!(cmd = %cmd, "PW plugin received command");
        match cmd {
            PluginCommand::GetState => Ok(PluginResponse::State(self.build_snapshot())),
            PluginCommand::CreateChannel { name } => {
                let id = self.next_channel_id;
                self.next_channel_id += 1;
                tracing::info!(channel_name = %name, channel_id = id, "PW creating channel");
                // TODO: create virtual sink node via PW API
                self.channels.push(ChannelInfo {
                    id,
                    name,
                    apps: vec![],
                    icon_path: None,
                    assigned_app_binaries: vec![],
                    muted: false,
                    effects: Default::default(),
                });
                self.effects_chains.insert(id, EffectsChain::new());
                Ok(PluginResponse::ChannelCreated { id })
            }
            PluginCommand::RemoveChannel { id } => {
                tracing::info!(channel_id = id, "PW removing channel");
                self.channels.retain(|c| c.id != id);
                self.effects_chains.remove(&id);
                self.routes
                    .retain(|(src, _), _| *src != SourceId::Channel(id));
                Ok(PluginResponse::Ok)
            }
            PluginCommand::RenameChannel { id, name } => {
                tracing::info!(channel_id = id, new_name = %name, "PW renaming channel");
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
                    ch.name = name;
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::CreateMix { name } => {
                let id = self.next_mix_id;
                self.next_mix_id += 1;
                tracing::info!(mix_name = %name, mix_id = id, "PW creating mix");
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
                tracing::info!(mix_id = id, "PW removing mix");
                self.mixes.retain(|m| m.id != id);
                self.routes.retain(|(_, mix), _| *mix != id);
                Ok(PluginResponse::Ok)
            }
            PluginCommand::RenameMix { id, name } => {
                tracing::info!(mix_id = id, new_name = %name, "PW renaming mix");
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
                tracing::debug!(?source, mix, volume, "PW set route volume");
                self.routes.entry((source, mix)).or_default().volume = volume.clamp(0.0, 1.0);
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetRouteEnabled {
                source,
                mix,
                enabled,
            } => {
                tracing::debug!(?source, mix, enabled, "PW set route enabled");
                if enabled {
                    self.routes.entry((source, mix)).or_default().enabled = true;
                    // TODO: create PW link between channel and mix nodes
                } else {
                    self.routes.remove(&(source, mix));
                    // TODO: remove PW link
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetRouteMuted { source, mix, muted } => {
                tracing::debug!(?source, mix, muted, "PW set route muted");
                self.routes.entry((source, mix)).or_default().muted = muted;
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetMixMasterVolume { mix, volume } => {
                tracing::debug!(mix, volume, "PW set mix master volume");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.master_volume = volume.clamp(0.0, 1.0);
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetMixMuted { mix, muted } => {
                tracing::debug!(mix, muted, "PW set mix muted");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.muted = muted;
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetSourceMuted { source, muted } => {
                tracing::debug!(?source, muted, "PW set source muted");
                if let SourceId::Channel(id) = source {
                    if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
                        ch.muted = muted;
                    }
                }
                for ((src, _), route) in &mut self.routes {
                    if *src == source {
                        route.muted = muted;
                    }
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::RouteApp { app, channel } => {
                tracing::debug!(app, channel, "PW route app");
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    if !ch.apps.contains(&app) {
                        ch.apps.push(app);
                    }
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::UnrouteApp { app } => {
                tracing::debug!(app, "PW unroute app");
                for ch in &mut self.channels {
                    ch.apps.retain(|&a| a != app);
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetMixOutput { mix, output } => {
                tracing::debug!(mix, output, "PW set mix output");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.output = Some(output);
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetEffectsParams { channel, params } => {
                tracing::debug!(channel, "PW set effects params");
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_params(params.clone());
                }
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects = params;
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetEffectsEnabled { channel, enabled } => {
                tracing::debug!(channel, enabled, "PW set effects enabled");
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_enabled(enabled);
                }
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects.enabled = enabled;
                }
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetAppVolume {
                stream_index,
                volume,
            } => {
                tracing::debug!(stream_index, volume, "PW set app volume (TODO)");
                Ok(PluginResponse::Ok)
            }
            PluginCommand::SetAppMuted {
                stream_index,
                muted,
            } => {
                tracing::debug!(stream_index, muted, "PW set app mute (TODO)");
                Ok(PluginResponse::Ok)
            }
            // Commands that are PA-specific but must still return Ok
            PluginCommand::ListHardwareInputs => Ok(PluginResponse::HardwareInputs(vec![])),
            PluginCommand::ListHardwareOutputs => Ok(PluginResponse::HardwareOutputs(vec![])),
            PluginCommand::ListApplications => Ok(PluginResponse::Applications(vec![])),
        }
    }

    fn poll_events(&mut self) -> Vec<PluginEvent> {
        Vec::new()
    }

    fn set_event_sender(&mut self, tx: std::sync::mpsc::Sender<crate::plugin::PluginThreadMsg>) {
        tracing::debug!("PW plugin received event sender");
        self.unified_tx = Some(tx);
        // TODO: spawn PW registry listener thread for events
    }

    fn cleanup(&mut self) -> Result<()> {
        tracing::info!("PipeWire plugin cleaning up");
        self.nodes.remove_all();
        if let Some(mut conn) = self.connection.take() {
            conn.disconnect();
        }
        tracing::info!("PipeWire plugin cleanup complete");
        Ok(())
    }
}
