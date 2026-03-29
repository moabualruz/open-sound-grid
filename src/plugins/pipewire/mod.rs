//! PipeWire plugin — native audio backend using PW nodes and links.
//!
//! Architecture:
//! - Each software channel → virtual null-audio-sink node (via PW factory)
//! - Each output mix → virtual null-audio-sink node
//! - Each (channel, mix) route → direct PW link (no loopback overhead)
//! - Volume control → wpctl set-volume (WirePlumber CLI)
//! - App routing → pw-metadata target.object
//! - Hardware/app enumeration → wpctl status + pw-cli
//!
//! Falls back to the PulseAudio plugin if PipeWire is not available.

#[cfg(feature = "pipewire-backend")]
mod connection;
#[cfg(feature = "pipewire-backend")]
mod nodes;
#[cfg(feature = "pipewire-backend")]
pub mod wpctl;

#[cfg(feature = "pipewire-backend")]
use std::collections::HashMap;

#[cfg(feature = "pipewire-backend")]
use crate::effects::EffectsChain;
#[cfg(feature = "pipewire-backend")]
use crate::error::{OsgError, Result};
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
    /// Maps channel_id → PW node ID for the channel's virtual sink.
    channel_pw_ids: HashMap<u32, u32>,
    /// Maps mix_id → PW node ID for the mix's virtual sink.
    mix_pw_ids: HashMap<u32, u32>,
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
            channel_pw_ids: HashMap::new(),
            mix_pw_ids: HashMap::new(),
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

        // Enumerate hardware and apps via wpctl/pw-cli
        let hardware_inputs = wpctl::list_hardware_inputs();
        let hardware_outputs = wpctl::list_hardware_outputs();
        let mut applications = wpctl::list_applications();

        // Populate AudioApplication.channel from channel.apps
        for app in &mut applications {
            for channel in &self.channels {
                if channel.apps.contains(&app.stream_index) {
                    app.channel = Some(channel.id);
                    break;
                }
            }
        }

        // Peak levels via wpctl get-volume for each monitored node
        let mut peak_levels = HashMap::new();
        for ch in &self.channels {
            if let Some(&pw_id) = self.channel_pw_ids.get(&ch.id) {
                let level = wpctl::get_volume(pw_id).unwrap_or(0.0);
                peak_levels.insert(SourceId::Channel(ch.id), level);
            }
        }
        for mx in &self.mixes {
            if let Some(&pw_id) = self.mix_pw_ids.get(&mx.id) {
                let level = wpctl::get_volume(pw_id).unwrap_or(0.0);
                peak_levels.insert(SourceId::Mix(mx.id), level);
            }
        }

        MixerSnapshot {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs,
            hardware_outputs,
            applications,
            peak_levels,
        }
    }

    /// Get the PW core, or return an error if not connected.
    fn core(&self) -> Result<&pipewire::core::CoreRc> {
        self.connection
            .as_ref()
            .map(|c| c.core())
            .ok_or_else(|| OsgError::PulseAudio("PipeWire not connected".into()))
    }

    /// Sink name for a channel (matches the PA convention for consistency).
    fn channel_sink_name(name: &str) -> String {
        format!("osg_{}_ch", name.replace(' ', "_"))
    }

    /// Sink name for a mix.
    fn mix_sink_name(name: &str) -> String {
        format!("osg_{}_mix", name.replace(' ', "_"))
    }
}

#[cfg(feature = "pipewire-backend")]
impl AudioPlugin for PipeWirePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pipewire",
            name: "PipeWire",
            version: "0.2.0",
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
            max_channels: None,
            max_mixes: None,
        }
    }

    fn init(&mut self) -> Result<()> {
        tracing::debug!("initializing PipeWire plugin");
        let conn = PwConnection::connect()?;
        tracing::info!("PipeWire plugin initialized (native backend)");
        self.connection = Some(conn);
        Ok(())
    }

    fn handle_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        tracing::debug!(cmd = %cmd, "PW plugin received command");
        match cmd {
            PluginCommand::GetState => Ok(PluginResponse::State(self.build_snapshot())),

            PluginCommand::CreateChannel { name } => {
                // Skip if already exists
                if let Some(ch) = self.channels.iter().find(|c| c.name == name) {
                    tracing::debug!(name = %name, "PW channel already exists");
                    return Ok(PluginResponse::ChannelCreated { id: ch.id });
                }
                let id = self.next_channel_id;
                self.next_channel_id += 1;

                let sink_name = Self::channel_sink_name(&name);
                let description = format!("OSG {name} Channel");

                // Create real PW virtual sink node
                let core = self.connection.as_ref().ok_or_else(|| OsgError::PulseAudio("PipeWire not connected".into()))?.core();
                match self.nodes.create_virtual_sink(
                    core,
                    &sink_name,
                    &description,
                ) {
                    Ok(_proxy_id) => {
                        // Roundtrip to ensure node is registered in PW registry
                        if let Some(conn) = &self.connection {
                            conn.do_roundtrip();
                        }
                        // Resolve the REAL global ID from wpctl (proxy ID is unreliable)
                        let global_id = wpctl::resolve_node_id_by_name(&sink_name)
                            .or_else(|| wpctl::resolve_node_id_by_name(&description));
                        if let Some(gid) = global_id {
                            tracing::info!(
                                channel_name = %name, channel_id = id,
                                global_id = gid, sink_name = %sink_name,
                                "PW channel virtual sink created (global ID resolved)"
                            );
                            self.channel_pw_ids.insert(id, gid);
                        } else {
                            tracing::error!(
                                channel_name = %name, sink_name = %sink_name,
                                "PW channel created but global ID not resolved — volume control will fail"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            channel_name = %name, err = %e,
                            "failed to create PW virtual sink — channel will have no audio routing"
                        );
                    }
                }

                self.channels.push(ChannelInfo {
                    id,
                    name,
                    apps: vec![],
                    icon_path: None,
                    assigned_app_binaries: vec![],
                    muted: false,
                    effects: Default::default(),
                    master_volume: 1.0,
                });
                self.effects_chains.insert(id, EffectsChain::new());
                Ok(PluginResponse::ChannelCreated { id })
            }

            PluginCommand::RemoveChannel { id } => {
                tracing::info!(channel_id = id, "PW removing channel");
                // Remove PW node
                self.nodes.remove_channel_node(id);
                self.channel_pw_ids.remove(&id);
                // Remove all route links involving this channel
                let source = SourceId::Channel(id);
                let link_keys: Vec<_> = self.routes.keys()
                    .filter(|(src, _)| *src == source)
                    .cloned()
                    .collect();
                for (_, mix_id) in &link_keys {
                    if let Some(&ch_pw_id) = self.channel_pw_ids.get(&id) {
                        self.nodes.remove_route_link(ch_pw_id, *mix_id);
                    }
                }
                self.channels.retain(|c| c.id != id);
                self.effects_chains.remove(&id);
                self.routes.retain(|(src, _), _| *src != source);
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
                if let Some(mx) = self.mixes.iter().find(|m| m.name == name) {
                    tracing::debug!(name = %name, "PW mix already exists");
                    return Ok(PluginResponse::MixCreated { id: mx.id });
                }
                let id = self.next_mix_id;
                self.next_mix_id += 1;

                let sink_name = Self::mix_sink_name(&name);
                let description = format!("OSG {name} Mix");

                let core = self.connection.as_ref().ok_or_else(|| OsgError::PulseAudio("PipeWire not connected".into()))?.core();
                match self.nodes.create_virtual_sink(
                    core,
                    &sink_name,
                    &description,
                ) {
                    Ok(_proxy_id) => {
                        if let Some(conn) = &self.connection {
                            conn.do_roundtrip();
                        }
                        let global_id = wpctl::resolve_node_id_by_name(&sink_name)
                            .or_else(|| wpctl::resolve_node_id_by_name(&description));
                        if let Some(gid) = global_id {
                            tracing::info!(
                                mix_name = %name, mix_id = id,
                                global_id = gid, sink_name = %sink_name,
                                "PW mix virtual sink created (global ID resolved)"
                            );
                            self.mix_pw_ids.insert(id, gid);
                        } else {
                            tracing::error!(
                                mix_name = %name, sink_name = %sink_name,
                                "PW mix created but global ID not resolved"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            mix_name = %name, err = %e,
                            "failed to create PW virtual sink for mix"
                        );
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
                tracing::info!(mix_id = id, "PW removing mix");
                self.nodes.remove_mix_node(id);
                self.nodes.remove_output_link(id);
                self.mix_pw_ids.remove(&id);
                // Remove route links targeting this mix
                let link_keys: Vec<_> = self.routes.keys()
                    .filter(|(_, mix)| *mix == id)
                    .cloned()
                    .collect();
                for (source, _) in &link_keys {
                    if let SourceId::Channel(ch_id) = source {
                        if let Some(&ch_pw_id) = self.channel_pw_ids.get(ch_id) {
                            self.nodes.remove_route_link(ch_pw_id, id);
                        }
                    }
                }
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

            PluginCommand::SetRouteEnabled { source, mix, enabled } => {
                tracing::debug!(?source, mix, enabled, "PW set route enabled");
                if enabled {
                    // Create direct PW link between channel and mix nodes
                    let ch_id = match source {
                        SourceId::Channel(id) => id,
                        _ => {
                            tracing::warn!(?source, "PW route enable: non-channel source not yet supported");
                            self.routes.entry((source, mix)).or_default().enabled = true;
                            return Ok(PluginResponse::Ok);
                        }
                    };
                    if let (Some(&ch_pw_id), Some(&mix_pw_id)) =
                        (self.channel_pw_ids.get(&ch_id), self.mix_pw_ids.get(&mix))
                    {
                        let core = self.connection.as_ref().ok_or_else(|| OsgError::PulseAudio("PipeWire not connected".into()))?.core();
                        match self.nodes.create_link(core, ch_pw_id, mix_pw_id) {
                            Ok(link_id) => {
                                tracing::info!(
                                    ?source, mix, ch_pw_id, mix_pw_id, link_id,
                                    "PW route link created (direct, zero-copy)"
                                );
                                if let Some(conn) = &self.connection {
                                    conn.do_roundtrip();
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    ?source, mix, err = %e,
                                    "failed to create PW route link"
                                );
                            }
                        }
                    } else {
                        tracing::warn!(
                            ?source, mix,
                            ch_exists = self.channel_pw_ids.contains_key(&ch_id),
                            mix_exists = self.mix_pw_ids.contains_key(&mix),
                            "PW route enable: missing node IDs"
                        );
                    }
                    self.routes.entry((source, mix)).or_default().enabled = true;
                } else {
                    // Remove the link
                    if let SourceId::Channel(ch_id) = source {
                        if let Some(&ch_pw_id) = self.channel_pw_ids.get(&ch_id) {
                            self.nodes.remove_route_link(ch_pw_id, mix);
                        }
                    }
                    self.routes.remove(&(source, mix));
                    tracing::debug!(?source, mix, "PW route disabled and link removed");
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteVolume { source, mix, volume } => {
                let volume = volume.clamp(0.0, 1.0);
                tracing::debug!(?source, mix, volume, "PW set route volume");
                let route = self.routes.entry((source, mix)).or_default();
                route.volume = volume;
                route.volume_left = volume;
                route.volume_right = volume;
                // Apply volume to the channel's PW node via wpctl
                if let SourceId::Channel(ch_id) = source {
                    if let Some(&pw_id) = self.channel_pw_ids.get(&ch_id) {
                        let _ = wpctl::set_volume(pw_id, volume);
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteStereoVolume { source, mix, left, right } => {
                let left = left.clamp(0.0, 1.0);
                let right = right.clamp(0.0, 1.0);
                tracing::debug!(?source, mix, left, right, "PW set route stereo volume");
                let route = self.routes.entry((source, mix)).or_default();
                route.volume_left = left;
                route.volume_right = right;
                route.volume = (left + right) / 2.0;
                // wpctl set-volume applies mono; for stereo, use the average
                // (PW native stereo would require SPA params, future work)
                if let SourceId::Channel(ch_id) = source {
                    if let Some(&pw_id) = self.channel_pw_ids.get(&ch_id) {
                        let _ = wpctl::set_volume(pw_id, route.volume);
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetRouteMuted { source, mix, muted } => {
                tracing::debug!(?source, mix, muted, "PW set route muted");
                self.routes.entry((source, mix)).or_default().muted = muted;
                // Mute the channel node
                if let SourceId::Channel(ch_id) = source {
                    if let Some(&pw_id) = self.channel_pw_ids.get(&ch_id) {
                        let _ = wpctl::set_mute(pw_id, muted);
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixMasterVolume { mix, volume } => {
                tracing::debug!(mix, volume, "PW set mix master volume");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.master_volume = volume.clamp(0.0, 1.0);
                    if let Some(&pw_id) = self.mix_pw_ids.get(&mix) {
                        let _ = wpctl::set_volume(pw_id, m.master_volume);
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixMuted { mix, muted } => {
                tracing::debug!(mix, muted, "PW set mix muted");
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.muted = muted;
                    if let Some(&pw_id) = self.mix_pw_ids.get(&mix) {
                        let _ = wpctl::set_mute(pw_id, muted);
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetSourceMuted { source, muted } => {
                tracing::debug!(?source, muted, "PW set source muted");
                if let SourceId::Channel(id) = source {
                    if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
                        ch.muted = muted;
                    }
                    if let Some(&pw_id) = self.channel_pw_ids.get(&id) {
                        let _ = wpctl::set_mute(pw_id, muted);
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
                // Move app stream to channel's virtual sink
                if let Some(&ch_pw_id) = self.channel_pw_ids.get(&channel) {
                    if let Err(e) = wpctl::move_stream_to_sink(app, ch_pw_id) {
                        tracing::warn!(app, channel, err = %e, "PW failed to route app");
                    }
                }
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    if !ch.apps.contains(&app) {
                        ch.apps.push(app);
                    }
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::UnrouteApp { app } => {
                tracing::debug!(app, "PW unroute app");
                // Move back to default: clear the target.object metadata
                if let Err(e) = wpctl::move_stream_to_sink(app, 0) {
                    tracing::warn!(app, err = %e, "PW failed to unroute app");
                }
                for ch in &mut self.channels {
                    ch.apps.retain(|&a| a != app);
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetMixOutput { mix, output } => {
                tracing::debug!(mix, output, "PW set mix output");
                // Create link from mix node to hardware output
                if let (Some(&mix_pw_id), Some(hw)) = (
                    self.mix_pw_ids.get(&mix),
                    self.build_snapshot().hardware_outputs.iter().find(|o| o.id == output),
                ) {
                    let hw_pw_id: u32 = hw.device_id.parse().unwrap_or(0);
                    if hw_pw_id > 0 {
                        // Remove old output link
                        self.nodes.remove_output_link(mix);
                        let core = self.connection.as_ref().ok_or_else(|| OsgError::PulseAudio("PipeWire not connected".into()))?.core();
                        match self.nodes.create_link(core, mix_pw_id, hw_pw_id) {
                            Ok(link_id) => {
                                tracing::info!(
                                    mix, mix_pw_id, hw_pw_id, link_id,
                                    "PW mix output link created"
                                );
                                if let Some(conn) = &self.connection {
                                    conn.do_roundtrip();
                                }
                            }
                            Err(e) => {
                                tracing::error!(mix, err = %e, "failed to create mix output link");
                            }
                        }
                    }
                }
                if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
                    m.output = Some(output);
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetEffectsParams { channel, params } => {
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_params(params.clone());
                }
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects = params;
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::SetEffectsEnabled { channel, enabled } => {
                if let Some(chain) = self.effects_chains.get_mut(&channel) {
                    chain.set_enabled(enabled);
                }
                if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
                    ch.effects.enabled = enabled;
                }
                Ok(PluginResponse::Ok)
            }

            PluginCommand::ListHardwareInputs => {
                Ok(PluginResponse::HardwareInputs(wpctl::list_hardware_inputs()))
            }
            PluginCommand::ListHardwareOutputs => {
                Ok(PluginResponse::HardwareOutputs(wpctl::list_hardware_outputs()))
            }
            PluginCommand::ListApplications => {
                Ok(PluginResponse::Applications(wpctl::list_applications()))
            }
        }
    }

    fn poll_events(&mut self) -> Vec<PluginEvent> {
        Vec::new()
    }

    fn set_event_sender(&mut self, tx: std::sync::mpsc::Sender<crate::plugin::PluginThreadMsg>) {
        tracing::debug!("PW plugin received event sender");
        self.unified_tx = Some(tx);
        // TODO: spawn PW registry listener thread for real-time events
        // For now, the app-side polling via GetState handles detection
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
