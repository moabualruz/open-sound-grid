//! Channel and mix lifecycle handlers: create, rename, remove, reorder, undo.

use crate::plugin::api::{ChannelId, MixId, PluginCommand};

use super::super::state::App;

impl App {
    pub fn handle_create_channel(&mut self, name: String) {
        let already_exists = self
            .engine
            .state
            .channels
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&name));
        if already_exists {
            tracing::warn!(name = %name, "channel already exists — skipping creation");
            self.show_channel_picker = false;
            self.show_channel_dropdown = false;
        } else {
            tracing::debug!(name = %name, "creating channel");
            self.show_channel_picker = false;
            self.show_channel_dropdown = false;
            self.engine
                .send_command(PluginCommand::CreateChannel { name });
            self.engine.send_command(PluginCommand::GetState);
        }
    }

    pub fn handle_create_mix(&mut self, name: String) {
        tracing::debug!(name = %name, "creating mix");
        self.engine.send_command(PluginCommand::CreateMix { name });
        self.engine.send_command(PluginCommand::GetState);
    }

    pub fn handle_start_rename_channel(&mut self, id: ChannelId) {
        tracing::debug!(channel_id = id, "starting channel rename");
        if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == id) {
            self.editing_text = ch.name.clone();
        }
        self.editing_channel = Some(id);
        self.editing_mix = None;
    }

    pub fn handle_start_rename_mix(&mut self, id: MixId) {
        tracing::debug!(mix_id = id, "starting mix rename");
        if let Some(mx) = self.engine.state.mixes.iter().find(|m| m.id == id) {
            self.editing_text = mx.name.clone();
        }
        self.editing_mix = Some(id);
        self.editing_channel = None;
    }

    pub fn handle_confirm_rename(&mut self) {
        let new_name = self.editing_text.trim().to_string();
        if !new_name.is_empty() {
            if let Some(id) = self.editing_channel.take() {
                tracing::info!(channel_id = id, name = %new_name, "renaming channel");
                self.engine
                    .send_command(PluginCommand::RenameChannel { id, name: new_name });
                self.engine.send_command(PluginCommand::GetState);
            } else if let Some(id) = self.editing_mix.take() {
                tracing::info!(mix_id = id, name = %new_name, "renaming mix");
                self.engine
                    .send_command(PluginCommand::RenameMix { id, name: new_name });
                self.engine.send_command(PluginCommand::GetState);
            }
        }
        self.editing_channel = None;
        self.editing_mix = None;
        self.editing_text.clear();
    }

    pub fn handle_cancel_rename(&mut self) {
        self.editing_channel = None;
        self.editing_mix = None;
        self.editing_text.clear();
    }

    pub fn handle_rename_channel(&mut self, id: ChannelId, name: String) {
        tracing::info!(channel_id = id, name = %name, "renaming channel (direct)");
        self.engine
            .send_command(PluginCommand::RenameChannel { id, name });
        self.engine.send_command(PluginCommand::GetState);
    }

    pub fn handle_rename_mix(&mut self, id: MixId, name: String) {
        tracing::info!(mix_id = id, name = %name, "renaming mix (direct)");
        self.engine
            .send_command(PluginCommand::RenameMix { id, name });
        self.engine.send_command(PluginCommand::GetState);
    }

    pub fn handle_remove_channel(&mut self, id: ChannelId) {
        if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == id) {
            self.undo_buffer = Some((ch.name.clone(), true));
        }
        tracing::info!(channel_id = id, "removing channel (undo available)");
        self.channel_master_volumes.remove(&id);
        self.engine
            .send_command(PluginCommand::RemoveChannel { id });
        self.engine.send_command(PluginCommand::GetState);
    }

    pub fn handle_remove_mix(&mut self, id: MixId) {
        if let Some(mx) = self.engine.state.mixes.iter().find(|m| m.id == id) {
            self.undo_buffer = Some((mx.name.clone(), false));
        }
        tracing::info!(mix_id = id, "removing mix (undo available)");
        self.engine.send_command(PluginCommand::RemoveMix { id });
        self.engine.send_command(PluginCommand::GetState);
    }

    pub fn handle_move_channel_up(&mut self, id: ChannelId) {
        if let Some(idx) = self.engine.state.channels.iter().position(|c| c.id == id) {
            if idx > 0 {
                self.engine.state.channels.swap(idx, idx - 1);
                tracing::debug!(channel_id = id, from = idx, to = idx - 1, "moved channel up");
            }
        }
    }

    pub fn handle_move_channel_down(&mut self, id: ChannelId) {
        if let Some(idx) = self.engine.state.channels.iter().position(|c| c.id == id) {
            if idx + 1 < self.engine.state.channels.len() {
                self.engine.state.channels.swap(idx, idx + 1);
                tracing::debug!(channel_id = id, from = idx, to = idx + 1, "moved channel down");
            }
        }
    }

    pub fn handle_undo_delete(&mut self) {
        if let Some((name, is_channel)) = self.undo_buffer.take() {
            if is_channel {
                tracing::info!(name = %name, "undoing channel deletion");
                self.engine
                    .send_command(PluginCommand::CreateChannel { name });
            } else {
                tracing::info!(name = %name, "undoing mix deletion");
                self.engine
                    .send_command(PluginCommand::CreateMix { name });
            }
            self.engine.send_command(PluginCommand::GetState);
        }
    }
}
