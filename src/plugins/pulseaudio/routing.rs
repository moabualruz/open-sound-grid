//! Routing command handlers: enable/disable routes, route/unroute apps, set mix output.

use crate::error::{OsgError, Result};
use crate::plugin::api::*;

use super::devices::DeviceEnumerator;
use super::PulseAudioPlugin;

impl PulseAudioPlugin {
    pub(crate) fn handle_set_route_enabled(
        &mut self,
        source: SourceId,
        mix: MixId,
        enabled: bool,
    ) -> Result<PluginResponse> {
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

    pub(crate) fn handle_route_app(
        &mut self,
        app: u32,
        channel: ChannelId,
    ) -> Result<PluginResponse> {
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

    pub(crate) fn handle_unroute_app(&mut self, app: u32) -> Result<PluginResponse> {
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

    pub(crate) fn handle_set_mix_output(
        &mut self,
        mix: MixId,
        output: OutputId,
    ) -> Result<PluginResponse> {
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
}
