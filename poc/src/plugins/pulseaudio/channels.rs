//! Channel command handlers: create, remove, rename.

use crate::effects::EffectsChain;
use crate::error::Result;
use crate::plugin::api::*;

use super::PulseAudioPlugin;

impl PulseAudioPlugin {
    pub(crate) fn handle_create_channel(&mut self, name: String) -> Result<PluginResponse> {
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

    pub(crate) fn handle_remove_channel(&mut self, id: ChannelId) -> Result<PluginResponse> {
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

    pub(crate) fn handle_rename_channel(
        &mut self,
        id: ChannelId,
        name: String,
    ) -> Result<PluginResponse> {
        tracing::info!(channel_id = id, new_name = %name, "renaming channel");
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
            ch.name = name;
        }
        Ok(PluginResponse::Ok)
    }
}
