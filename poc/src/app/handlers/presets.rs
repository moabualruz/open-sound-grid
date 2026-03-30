//! Preset save/load/input handlers.

use iced::Task;

use crate::config::RouteConfig;
use crate::plugin::api::{ChannelId, MixId, PluginCommand};

use super::super::messages::Message;
use super::super::state::App;

impl App {
    pub fn handle_save_preset(&mut self, name: String) -> Task<Message> {
        tracing::info!(name = %name, "saving preset");
        let preset = crate::presets::MixerPreset::from_current(
            &name,
            &self.config,
            &self.engine.state,
        );
        if let Err(e) = preset.save() {
            tracing::error!(error = %e, "failed to save preset");
        }
        self.available_presets = crate::presets::MixerPreset::list();
        Task::none()
    }

    pub fn handle_load_preset(&mut self, name: String) -> Task<Message> {
        tracing::info!(name = %name, "loading preset");
        match crate::presets::MixerPreset::load(&name) {
            Ok(preset) => {
                tracing::debug!(
                    channels = preset.channels.len(),
                    mixes = preset.mixes.len(),
                    routes = preset.routes.len(),
                    "preset loaded, removing old state before restore"
                );

                // Remove channels/mixes that are NOT in the new preset
                // (keeps anything that matches by name — avoids teardown+recreate)
                let preset_ch_names: Vec<&str> =
                    preset.channels.iter().map(|c| c.name.as_str()).collect();
                let preset_mx_names: Vec<&str> =
                    preset.mixes.iter().map(|m| m.name.as_str()).collect();

                let channels_to_remove: Vec<ChannelId> = self
                    .engine
                    .state
                    .channels
                    .iter()
                    .filter(|c| !preset_ch_names.contains(&c.name.as_str()))
                    .map(|c| c.id)
                    .collect();
                let mixes_to_remove: Vec<MixId> = self
                    .engine
                    .state
                    .mixes
                    .iter()
                    .filter(|m| {
                        !preset_mx_names.contains(&m.name.as_str())
                            && m.name != "Main/Monitor"
                    })
                    .map(|m| m.id)
                    .collect();

                for id in channels_to_remove {
                    tracing::debug!(channel_id = id, "removing channel for preset load");
                    self.engine
                        .send_command(PluginCommand::RemoveChannel { id });
                }
                for id in mixes_to_remove {
                    tracing::debug!(mix_id = id, "removing mix for preset load");
                    self.engine.send_command(PluginCommand::RemoveMix { id });
                }

                self.config.channels = preset.channels;
                self.config.mixes = preset.mixes;
                self.pending_route_restores = preset
                    .routes
                    .iter()
                    .map(|r| {
                        RouteConfig {
                            channel_name: r.channel_name.clone(),
                            mix_name: r.mix_name.clone(),
                            volume: r.volume,
                            enabled: r.enabled,
                            muted: r.muted,
                            volume_left: r.volume_left,
                            volume_right: r.volume_right,
                        }
                    })
                    .collect();
                tracing::debug!(
                    count = self.pending_route_restores.len(),
                    "route restores queued for next StateRefreshed"
                );
                // Reset route initialization so routes are recreated for new channels
                self.auto_routes_sent = false;
                self.routes_initialized.clear();
                tracing::debug!("cleared routes_initialized for preset load");
                let _ = self.config.save();
                self.restore_from_config();
            }
            Err(e) => tracing::error!(error = %e, "failed to load preset"),
        }
        Task::none()
    }

    pub fn handle_preset_name_input(&mut self, text: String) -> Task<Message> {
        self.preset_name_input = text;
        Task::none()
    }
}
