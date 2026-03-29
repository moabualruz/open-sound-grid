//! Keyboard shortcut handler — extracted from the main update loop.

use iced::keyboard::Key;
use iced::keyboard::Modifiers;
use iced::Task;

use crate::plugin::api::{PluginCommand, SourceId};

use super::super::messages::Message;
use super::super::state::App;

impl App {
    pub(crate) fn handle_key_pressed(
        &mut self,
        key: Key,
        modifiers: Modifiers,
    ) -> Task<Message> {
        match key {
            Key::Named(iced::keyboard::key::Named::Tab) => {
                let max_col = self.engine.state.mixes.len();
                let max_row = self.engine.state.channels.len();
                if max_col == 0 || max_row == 0 {
                    return Task::none();
                }
                let (r, c) = match (self.focused_row, self.focused_col) {
                    (Some(r), Some(c)) => {
                        if modifiers.shift() {
                            if c > 0 {
                                (r, c - 1)
                            } else if r > 0 {
                                (r - 1, max_col - 1)
                            } else {
                                (max_row - 1, max_col - 1)
                            }
                        } else {
                            if c + 1 < max_col {
                                (r, c + 1)
                            } else if r + 1 < max_row {
                                (r + 1, 0)
                            } else {
                                (0, 0)
                            }
                        }
                    }
                    _ => (0, 0),
                };
                self.focused_row = Some(r);
                self.focused_col = Some(c);
                tracing::debug!(row = r, col = c, "keyboard: cell focused");
            }
            Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                // Use WL3 model: adjust cell ratio, not raw PA volume
                if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                    if let (Some(ch), Some(mix)) = (
                        self.engine.state.channels.get(r),
                        self.engine.state.mixes.get(c),
                    ) {
                        let source = SourceId::Channel(ch.id);
                        let current_ratio = self
                            .engine
                            .state
                            .route_ratios
                            .get(&(source, mix.id))
                            .copied()
                            .unwrap_or(1.0);
                        let new_ratio = (current_ratio + 0.01).min(1.0);
                        tracing::debug!(
                            channel_id = ch.id, mix_id = mix.id,
                            old_ratio = current_ratio, new_ratio,
                            "keyboard: volume up (WL3 ratio)"
                        );
                        return self.update(Message::RouteVolumeChanged {
                            source,
                            mix: mix.id,
                            volume: new_ratio,
                        });
                    }
                }
            }
            Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                // Use WL3 model: adjust cell ratio, not raw PA volume
                if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                    if let (Some(ch), Some(mix)) = (
                        self.engine.state.channels.get(r),
                        self.engine.state.mixes.get(c),
                    ) {
                        let source = SourceId::Channel(ch.id);
                        let current_ratio = self
                            .engine
                            .state
                            .route_ratios
                            .get(&(source, mix.id))
                            .copied()
                            .unwrap_or(1.0);
                        let new_ratio = (current_ratio - 0.01).max(0.0);
                        tracing::debug!(
                            channel_id = ch.id, mix_id = mix.id,
                            old_ratio = current_ratio, new_ratio,
                            "keyboard: volume down (WL3 ratio)"
                        );
                        return self.update(Message::RouteVolumeChanged {
                            source,
                            mix: mix.id,
                            volume: new_ratio,
                        });
                    }
                }
            }
            Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                if let Some(c) = self.focused_col {
                    if c > 0 {
                        self.focused_col = Some(c - 1);
                        tracing::debug!(col = c - 1, "keyboard: focus moved left");
                    }
                }
            }
            Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                let max_col = self.engine.state.mixes.len();
                if let Some(c) = self.focused_col {
                    if c + 1 < max_col {
                        self.focused_col = Some(c + 1);
                        tracing::debug!(col = c + 1, "keyboard: focus moved right");
                    }
                }
            }
            Key::Character(ref ch) if ch.as_str() == "m" || ch.as_str() == "M" => {
                tracing::debug!(
                    focused_row = ?self.focused_row,
                    focused_col = ?self.focused_col,
                    "keyboard: toggle mute"
                );
                if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                    if let (Some(channel), Some(mix)) = (
                        self.engine.state.channels.get(r),
                        self.engine.state.mixes.get(c),
                    ) {
                        let source = SourceId::Channel(channel.id);
                        let currently_muted = self
                            .engine
                            .state
                            .routes
                            .get(&(source, mix.id))
                            .map_or(false, |r| r.muted);
                        tracing::debug!(
                            row = r,
                            col = c,
                            channel_id = channel.id,
                            mix_id = mix.id,
                            new_muted = !currently_muted,
                            "keyboard: mute toggled"
                        );
                        self.engine.send_command(PluginCommand::SetRouteMuted {
                            source,
                            mix: mix.id,
                            muted: !currently_muted,
                        });
                    }
                }
            }
            Key::Named(iced::keyboard::key::Named::Space) => {
                tracing::debug!(
                    focused_row = ?self.focused_row,
                    focused_col = ?self.focused_col,
                    "keyboard: toggle route"
                );
                if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                    if let (Some(channel), Some(mix)) = (
                        self.engine.state.channels.get(r),
                        self.engine.state.mixes.get(c),
                    ) {
                        let source = SourceId::Channel(channel.id);
                        let enabled = self
                            .engine
                            .state
                            .routes
                            .get(&(source, mix.id))
                            .map_or(true, |r| r.enabled);
                        tracing::debug!(
                            row = r,
                            col = c,
                            channel_id = channel.id,
                            mix_id = mix.id,
                            new_enabled = !enabled,
                            "keyboard: route enabled toggled"
                        );
                        self.engine.send_command(PluginCommand::SetRouteEnabled {
                            source,
                            mix: mix.id,
                            enabled: !enabled,
                        });
                    }
                }
            }
            Key::Named(iced::keyboard::key::Named::Escape) => {
                self.focused_row = None;
                self.focused_col = None;
                tracing::debug!("keyboard: focus cleared");
            }
            // Enter = toggle route (same as Space)
            Key::Named(iced::keyboard::key::Named::Enter) => {
                tracing::debug!("keyboard: Enter = toggle route");
                if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                    if let (Some(channel), Some(mix)) = (
                        self.engine.state.channels.get(r),
                        self.engine.state.mixes.get(c),
                    ) {
                        let source = SourceId::Channel(channel.id);
                        let enabled = self
                            .engine
                            .state
                            .routes
                            .get(&(source, mix.id))
                            .map_or(true, |r| r.enabled);
                        self.engine.send_command(PluginCommand::SetRouteEnabled {
                            source,
                            mix: mix.id,
                            enabled: !enabled,
                        });
                    }
                }
            }
            // Number keys 1-5 = load preset by index
            Key::Character(ref ch)
                if !modifiers.control()
                    && !modifiers.alt()
                    && matches!(ch.as_str(), "1" | "2" | "3" | "4" | "5") =>
            {
                let idx: usize = ch.as_str().parse::<usize>().unwrap_or(1) - 1;
                if let Some(preset_name) = self.available_presets.get(idx) {
                    let name = preset_name.clone();
                    tracing::info!(index = idx + 1, preset = %name, "keyboard: loading preset by number key");
                    return self.update(Message::LoadPreset(name));
                } else {
                    tracing::debug!(index = idx + 1, "keyboard: no preset at this index");
                }
            }
            // v0.4.0: Ctrl+C/V for effects copy/paste
            Key::Character(ref ch)
                if (ch.as_str() == "c" || ch.as_str() == "C") && modifiers.control() =>
            {
                if let Some(ch_id) = self.selected_channel {
                    tracing::info!(channel_id = ch_id, "keyboard: copy effects (Ctrl+C)");
                    return self.update(Message::CopyEffects(ch_id));
                }
            }
            Key::Character(ref ch)
                if (ch.as_str() == "v" || ch.as_str() == "V") && modifiers.control() =>
            {
                if let Some(ch_id) = self.selected_channel {
                    tracing::info!(channel_id = ch_id, "keyboard: paste effects (Ctrl+V)");
                    return self.update(Message::PasteEffects(ch_id));
                }
            }
            _ => {}
        }
        Task::none()
    }
}
