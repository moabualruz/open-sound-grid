//! Tray and global hotkey message handlers.

use iced::Task;

use crate::plugin::api::{PluginCommand, SourceId};

use super::super::messages::Message;
use super::super::state::App;

impl App {
    pub fn handle_tray_show(&mut self) -> Task<Message> {
        tracing::info!("tray: show window requested");
        // iced doesn't have a show/hide window API in 0.14 —
        // the tray "Show" is a no-op for now (window is always visible)
        Task::none()
    }

    pub fn handle_tray_quit(&mut self) -> Task<Message> {
        tracing::info!("tray: quit requested — auto-saving session preset + config");
        // Auto-save "Last Session" preset so next launch can restore exact state
        let preset = crate::presets::MixerPreset::from_current(
            "_last_session",
            &self.config,
            &self.engine.state,
        );
        if let Err(e) = preset.save() {
            tracing::warn!(error = %e, "failed to auto-save last session preset");
        } else {
            tracing::debug!("auto-saved _last_session preset");
        }
        let _ = self.config.save();
        iced::exit()
    }

    pub fn handle_mute_all(&mut self) -> Task<Message> {
        tracing::info!("mute all requested (tray or hotkey)");
        for channel in &self.engine.state.channels {
            self.engine.send_command(PluginCommand::SetSourceMuted {
                source: SourceId::Channel(channel.id),
                muted: true,
            });
        }
        Task::none()
    }
}
