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
mod channels;
mod connection;
mod devices;
mod effects_handler;
mod introspect;
mod introspect_control;
mod lifecycle;
mod mixes;
mod modules;
mod peaks;
mod routing;
mod snapshots;
pub mod spectrum;
mod volume;

use std::collections::HashMap;
use std::process::Child;
use std::sync::mpsc as std_mpsc;

use crate::effects::EffectsChain;
use crate::error::Result;
use crate::plugin::api::*;

use self::apps::AppDetector;
use self::connection::PulseConnection;
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

    /// Inner dispatch: execute a plugin command and return the result.
    /// Called by `handle_command` which adds the error-level tracing wrapper.
    fn dispatch_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse> {
        match cmd {
            PluginCommand::GetState => {
                tracing::debug!("building mixer snapshot");
                Ok(PluginResponse::State(self.build_snapshot()))
            }

            PluginCommand::ListHardwareInputs => Ok(PluginResponse::HardwareInputs(
                devices::DeviceEnumerator::list_inputs(self.connection.as_mut()),
            )),

            PluginCommand::ListHardwareOutputs => Ok(PluginResponse::HardwareOutputs(
                devices::DeviceEnumerator::list_outputs(self.connection.as_mut()),
            )),

            PluginCommand::ListApplications => {
                let apps = self.apps.list_applications(self.connection.as_mut())?;
                tracing::debug!(count = apps.len(), "listing applications");
                Ok(PluginResponse::Applications(apps))
            }

            PluginCommand::CreateChannel { name } => self.handle_create_channel(name),
            PluginCommand::RemoveChannel { id } => self.handle_remove_channel(id),
            PluginCommand::RenameChannel { id, name } => self.handle_rename_channel(id, name),

            PluginCommand::CreateMix { name } => self.handle_create_mix(name),
            PluginCommand::RemoveMix { id } => self.handle_remove_mix(id),
            PluginCommand::RenameMix { id, name } => self.handle_rename_mix(id, name),

            PluginCommand::SetRouteVolume { source, mix, volume } => {
                self.handle_set_route_volume(source, mix, volume)
            }
            PluginCommand::SetRouteEnabled { source, mix, enabled } => {
                self.handle_set_route_enabled(source, mix, enabled)
            }
            PluginCommand::SetRouteMuted { source, mix, muted } => {
                self.handle_set_route_muted(source, mix, muted)
            }

            PluginCommand::RouteApp { app, channel } => self.handle_route_app(app, channel),
            PluginCommand::UnrouteApp { app } => self.handle_unroute_app(app),
            PluginCommand::SetMixOutput { mix, output } => {
                self.handle_set_mix_output(mix, output)
            }

            PluginCommand::SetMixMasterVolume { mix, volume } => {
                self.handle_set_mix_master_volume(mix, volume)
            }
            PluginCommand::SetMixMuted { mix, muted } => self.handle_set_mix_muted(mix, muted),
            PluginCommand::SetSourceMuted { source, muted } => {
                self.handle_set_source_muted(source, muted)
            }

            PluginCommand::SetRouteStereoVolume {
                source,
                mix,
                left,
                right,
            } => self.handle_set_route_stereo_volume(source, mix, left, right),
            PluginCommand::SetEffectsParams { channel, params } => {
                self.handle_set_effects_params(channel, params)
            }
            PluginCommand::SetEffectsEnabled { channel, enabled } => {
                self.handle_set_effects_enabled(channel, enabled)
            }
        }
    }
}
