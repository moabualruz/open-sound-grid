//! Output device selection handlers.

use iced::Task;

use crate::plugin::api::{MixId, PluginCommand};

use super::super::messages::Message;
use super::super::state::App;

impl App {
    pub fn handle_mix_output_device_selected(
        &mut self,
        mix_id: MixId,
        device_name: String,
    ) -> Task<Message> {
        tracing::debug!(mix_id, device_name = %device_name, "mix output device selected");

        if device_name == "None" {
            // Unset output device for this mix
            tracing::info!(mix_id, "clearing mix output device");
            if let Some(mix_config) = self.config.mixes.iter_mut().find(|c| {
                self.engine
                    .state
                    .mixes
                    .iter()
                    .any(|m| m.id == mix_id && m.name == c.name)
            }) {
                mix_config.output_device = None;
                let _ = self.config.save();
            }
            // Update engine state
            if let Some(m) = self.engine.state.mixes.iter_mut().find(|m| m.id == mix_id) {
                m.output = None;
            }
        } else {
            let hw_output = self
                .engine
                .state
                .hardware_outputs
                .iter()
                .find(|o| o.name == device_name)
                .cloned();
            if let Some(output) = hw_output {
                tracing::info!(
                    mix_id,
                    output_id = output.id,
                    output_name = %output.name,
                    "setting mix output device"
                );
                self.engine.send_command(PluginCommand::SetMixOutput {
                    mix: mix_id,
                    output: output.id,
                });
                // Persist the selection to config
                if let Some(mix_config) = self.config.mixes.iter_mut().find(|c| {
                    self.engine
                        .state
                        .mixes
                        .iter()
                        .any(|m| m.id == mix_id && m.name == c.name)
                }) {
                    mix_config.output_device = Some(device_name.clone());
                    if let Err(e) = self.config.save() {
                        tracing::error!(
                            error = %e,
                            "failed to save output device config"
                        );
                    }
                }
            } else {
                tracing::warn!(
                    device_name = %device_name,
                    "MixOutputDeviceSelected: device not found"
                );
            }
        }
        Task::none()
    }
}
