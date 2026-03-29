//! Mix command handlers: create, remove, rename.

use crate::error::Result;
use crate::plugin::api::*;

use super::PulseAudioPlugin;

impl PulseAudioPlugin {
    pub(crate) fn handle_create_mix(&mut self, name: String) -> Result<PluginResponse> {
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

    pub(crate) fn handle_remove_mix(&mut self, id: MixId) -> Result<PluginResponse> {
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

    pub(crate) fn handle_rename_mix(
        &mut self,
        id: MixId,
        name: String,
    ) -> Result<PluginResponse> {
        tracing::info!(mix_id = id, new_name = %name, "renaming mix");
        if let Some(mx) = self.mixes.iter_mut().find(|m| m.id == id) {
            mx.name = name;
        }
        Ok(PluginResponse::Ok)
    }
}
