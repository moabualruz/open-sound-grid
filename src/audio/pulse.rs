use crate::audio::{
    AppId, AudioApplication, AudioBackend, ChannelId, ChannelState, HardwareInput, HardwareOutput,
    MixId, MixState, MixerState, OutputId, RouteState, SourceId,
};
use crate::error::{OsgError, Result};
use std::collections::HashMap;

/// PulseAudio backend implementation.
///
/// Uses null sinks for channels, null sinks for mixes,
/// and module-loopback to connect them. Volume control
/// is done via sink-input volume on the loopback instances.
pub struct PulseAudioBackend {
    connected: bool,
    next_channel_id: u32,
    next_mix_id: u32,
    channels: Vec<ChannelState>,
    mixes: Vec<MixState>,
    routes: HashMap<(SourceId, MixId), RouteState>,
}

impl PulseAudioBackend {
    pub fn new() -> Self {
        Self {
            connected: false,
            next_channel_id: 1,
            next_mix_id: 1,
            channels: Vec::new(),
            mixes: Vec::new(),
            routes: HashMap::new(),
        }
    }
}

impl AudioBackend for PulseAudioBackend {
    fn init(&mut self) -> Result<()> {
        // TODO: Connect to PulseAudio server via libpulse-binding
        // - Create threaded mainloop
        // - Connect context
        // - Subscribe to events (sink, sink-input, source changes)
        tracing::info!("PulseAudio backend initialized (stub)");
        self.connected = true;
        Ok(())
    }

    fn get_state(&self) -> Result<MixerState> {
        Ok(MixerState {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs: self.list_hardware_inputs()?,
            hardware_outputs: self.list_hardware_outputs()?,
            applications: self.list_applications()?,
            peak_levels: HashMap::new(),
        })
    }

    fn list_hardware_inputs(&self) -> Result<Vec<HardwareInput>> {
        // TODO: pactl list short sources | filter monitors
        Ok(vec![])
    }

    fn list_hardware_outputs(&self) -> Result<Vec<HardwareOutput>> {
        // TODO: pactl list short sinks | filter virtual
        Ok(vec![])
    }

    fn list_applications(&self) -> Result<Vec<AudioApplication>> {
        // TODO: pactl list sink-inputs with application.name
        Ok(vec![])
    }

    fn create_channel(&mut self, name: &str) -> Result<ChannelId> {
        let id = ChannelId(self.next_channel_id);
        self.next_channel_id += 1;

        // TODO: pactl load-module module-null-sink sink_name={name}_Apps
        tracing::info!("Created channel '{}' with id {}", name, id);

        self.channels.push(ChannelState {
            id,
            name: name.to_string(),
            apps: Vec::new(),
            muted: false,
        });

        Ok(id)
    }

    fn remove_channel(&mut self, id: ChannelId) -> Result<()> {
        // TODO: unload null sink module and all associated loopbacks
        self.channels.retain(|c| c.id != id);
        self.routes.retain(|(src, _), _| *src != SourceId::Channel(id));
        Ok(())
    }

    fn create_mix(&mut self, name: &str) -> Result<MixId> {
        let id = MixId(self.next_mix_id);
        self.next_mix_id += 1;

        // TODO: pactl load-module module-null-sink sink_name={name}_Mix
        // TODO: create loopbacks from each existing channel to this mix
        tracing::info!("Created mix '{}' with id {}", name, id);

        self.mixes.push(MixState {
            id,
            name: name.to_string(),
            icon: "🎧".to_string(),
            color: [100, 149, 237], // cornflower blue
            output: None,
            master_volume: 1.0,
            muted: false,
        });

        Ok(id)
    }

    fn remove_mix(&mut self, id: MixId) -> Result<()> {
        // TODO: unload mix null sink and associated loopbacks
        self.mixes.retain(|m| m.id != id);
        self.routes.retain(|(_, mix), _| *mix != id);
        Ok(())
    }

    fn set_route_volume(&mut self, source: SourceId, mix: MixId, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);

        // TODO: pactl set-sink-input-volume <loopback_idx> <volume>%
        self.routes
            .entry((source, mix))
            .or_default()
            .volume = volume;

        Ok(())
    }

    fn set_route_enabled(&mut self, source: SourceId, mix: MixId, enabled: bool) -> Result<()> {
        // TODO: load/unload the loopback module for this route
        self.routes
            .entry((source, mix))
            .or_default()
            .enabled = enabled;

        Ok(())
    }

    fn route_app_to_channel(&mut self, app: AppId, channel: ChannelId) -> Result<()> {
        // TODO: pactl move-sink-input <app_sink_input_idx> <channel_sink_name>
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
            if !ch.apps.contains(&app) {
                ch.apps.push(app);
            }
            Ok(())
        } else {
            Err(OsgError::ChannelNotFound(channel.0))
        }
    }

    fn set_mix_output(&mut self, mix: MixId, output: OutputId) -> Result<()> {
        // TODO: create loopback from mix monitor → output device
        if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
            m.output = Some(output);
            Ok(())
        } else {
            Err(OsgError::MixNotFound(mix.0))
        }
    }

    fn set_mix_master_volume(&mut self, mix: MixId, volume: f32) -> Result<()> {
        // TODO: pactl set-sink-volume <mix_sink_name> <volume>%
        if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
            m.master_volume = volume.clamp(0.0, 1.0);
            Ok(())
        } else {
            Err(OsgError::MixNotFound(mix.0))
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        // TODO: unload all modules we created, disconnect from PA
        tracing::info!("PulseAudio backend cleanup (stub)");
        self.connected = false;
        Ok(())
    }
}
