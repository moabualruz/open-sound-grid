//! Effects toggle, parameter, copy/paste handlers.

use iced::Task;

use crate::plugin::api::{ChannelId, PluginCommand};
use crate::ui;

use super::super::messages::Message;
use super::super::state::App;

impl App {
    pub fn handle_effects_toggled(
        &mut self,
        channel: ChannelId,
        enabled: bool,
    ) -> Task<Message> {
        tracing::debug!(channel_id = channel, enabled, "effects toggled");
        self.engine
            .send_command(PluginCommand::SetEffectsEnabled { channel, enabled });
        Task::none()
    }

    pub fn handle_effects_param_changed(
        &mut self,
        channel: ChannelId,
        param: String,
        value: f32,
    ) -> Task<Message> {
        tracing::debug!(channel_id = channel, param = %param, value, "effects param changed");
        // Find current params for this channel, apply the change, send update
        if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == channel) {
            if let Some(new_params) =
                ui::effects_panel::apply_param_change(&ch.effects, &param, value)
            {
                self.engine.send_command(PluginCommand::SetEffectsParams {
                    channel,
                    params: new_params,
                });
            } else {
                tracing::warn!(param = %param, "EffectsParamChanged: unknown param name");
            }
        } else {
            tracing::warn!(
                channel_id = channel,
                "EffectsParamChanged: channel not found in state"
            );
        }
        Task::none()
    }

    pub fn handle_copy_effects(&mut self, channel_id: ChannelId) -> Task<Message> {
        if let Some(ch) = self
            .engine
            .state
            .channels
            .iter()
            .find(|c| c.id == channel_id)
        {
            tracing::info!(channel_id, name = %ch.name, "copied effects");
            self.copied_effects = Some(ch.effects.clone());
        }
        Task::none()
    }

    pub fn handle_paste_effects(&mut self, channel_id: ChannelId) -> Task<Message> {
        if let Some(params) = self.copied_effects.clone() {
            tracing::info!(channel_id, "pasting effects");
            self.engine.send_command(PluginCommand::SetEffectsParams {
                channel: channel_id,
                params,
            });
        }
        Task::none()
    }
}
