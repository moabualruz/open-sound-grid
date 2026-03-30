//! Handlers for app routing and assignment messages.

use crate::plugin::api::{ChannelId, PluginCommand};

use super::super::state::App;

impl App {
    /// Handle `Message::AssignApp { channel, stream_index }`.
    pub(crate) fn handle_assign_app(&mut self, channel: ChannelId, stream_index: u32) {
        tracing::info!(
            stream_index,
            channel_id = channel,
            "assigning app to channel"
        );
        // Persist app identifier in config (binary if available, else name)
        if let Some(app_info) = self
            .engine
            .state
            .applications
            .iter()
            .find(|a| a.stream_index == stream_index)
        {
            let binary = if app_info.binary.is_empty() {
                app_info.name.clone()
            } else {
                app_info.binary.clone()
            };

            // Remove from any other channel first (no duplicates across channels)
            for ch_cfg in &mut self.config.channels {
                let target_name = self
                    .engine
                    .state
                    .channels
                    .iter()
                    .find(|ch| ch.id == channel)
                    .map(|ch| ch.name.as_str());
                if target_name.map_or(true, |n| n != ch_cfg.name) {
                    if ch_cfg.assigned_apps.contains(&binary) {
                        tracing::info!(
                            binary = %binary,
                            old_channel = %ch_cfg.name,
                            "removing app from previous channel before reassignment"
                        );
                        ch_cfg.assigned_apps.retain(|b| b != &binary);
                    }
                }
            }

            // Add to target channel config
            if let Some(ch_cfg) = self.config.channels.iter_mut().find(|c| {
                self.engine
                    .state
                    .channels
                    .iter()
                    .find(|ch| ch.id == channel)
                    .map(|ch| ch.name == c.name)
                    .unwrap_or(false)
            }) {
                if !ch_cfg.assigned_apps.contains(&binary) {
                    ch_cfg.assigned_apps.push(binary);
                }
            }
            let _ = self.config.save();
        }
        self.engine.send_command(PluginCommand::RouteApp {
            app: stream_index,
            channel,
        });

        // Remove the app's solo/auto channel if it exists.
        // Solo channels have the same name as the app and were auto-created.
        if let Some(app_info) = self
            .engine
            .state
            .applications
            .iter()
            .find(|a| a.stream_index == stream_index)
        {
            let app_name = app_info.name.clone();
            let target_ch_name = self
                .engine
                .state
                .channels
                .iter()
                .find(|ch| ch.id == channel)
                .map(|ch| ch.name.clone());
            // Find a solo channel matching the app name (but not the target channel)
            if let Some(solo_ch) = self
                .engine
                .state
                .channels
                .iter()
                .find(|c| {
                    c.name.eq_ignore_ascii_case(&app_name)
                        && target_ch_name
                            .as_ref()
                            .map_or(true, |t| !c.name.eq_ignore_ascii_case(t))
                })
            {
                tracing::info!(
                    solo_channel = %solo_ch.name,
                    solo_id = solo_ch.id,
                    "removing solo channel — app was grouped"
                );
                let solo_id = solo_ch.id;
                self.engine
                    .send_command(PluginCommand::RemoveChannel { id: solo_id });
                // Also remove from config
                self.config
                    .channels
                    .retain(|c| !c.name.eq_ignore_ascii_case(&app_name));
                self.channel_master_volumes.remove(&solo_id);
                let _ = self.config.save();
            }
        }
        self.engine.send_command(PluginCommand::GetState);
    }

    /// Handle `Message::UnassignApp { channel, stream_index }`.
    pub(crate) fn handle_unassign_app(&mut self, channel: ChannelId, stream_index: u32) {
        tracing::info!(
            stream_index,
            channel_id = channel,
            "unassigning app from channel"
        );
        // Remove app identifier from config (binary or name fallback)
        if let Some(app_info) = self
            .engine
            .state
            .applications
            .iter()
            .find(|a| a.stream_index == stream_index)
        {
            let key = if app_info.binary.is_empty() {
                &app_info.name
            } else {
                &app_info.binary
            };
            let binary = key;
            if let Some(ch_cfg) = self.config.channels.iter_mut().find(|c| {
                self.engine
                    .state
                    .channels
                    .iter()
                    .find(|ch| ch.id == channel)
                    .map(|ch| ch.name == c.name)
                    .unwrap_or(false)
            }) {
                ch_cfg.assigned_apps.retain(|b| b != binary);
                let _ = self.config.save();
            }
        }
        self.engine
            .send_command(PluginCommand::UnrouteApp { app: stream_index });
        self.engine.send_command(PluginCommand::GetState);
    }

    /// Handle `Message::CreateChannelFromApp(stream_index)`.
    pub(crate) fn handle_create_channel_from_app(&mut self, stream_index: u32) {
        if let Some(app) = self
            .engine
            .state
            .applications
            .iter()
            .find(|a| a.stream_index == stream_index)
        {
            let name = app.name.clone();
            let binary = app.binary.clone();
            // If a channel with this name already exists, just assign the app to it
            let existing = self
                .engine
                .state
                .channels
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&name))
                .map(|c| c.id);
            if let Some(ch_id) = existing {
                tracing::info!(stream_index, name = %name, channel_id = ch_id, "app channel already exists — assigning app");
                self.engine.send_command(PluginCommand::RouteApp {
                    app: stream_index,
                    channel: ch_id,
                });
                // Persist binary in config
                if let Some(ch_cfg) = self
                    .config
                    .channels
                    .iter_mut()
                    .find(|c| c.name.eq_ignore_ascii_case(&name))
                {
                    if !binary.is_empty() && !ch_cfg.assigned_apps.contains(&binary) {
                        ch_cfg.assigned_apps.push(binary);
                        let _ = self.config.save();
                    }
                }
            } else {
                tracing::info!(stream_index, name = %name, binary = %binary, "creating channel from detected app");
                // Pre-add the app to config so it persists across restarts
                let app_key = if binary.is_empty() {
                    name.clone()
                } else {
                    binary
                };
                self.config.channels.push(crate::config::ChannelConfig {
                    name: name.clone(),
                    effects: Default::default(),
                    muted: false,
                    assigned_apps: vec![app_key],
                    master_volume: 1.0,
                });
                let _ = self.config.save();
                self.engine
                    .send_command(PluginCommand::CreateChannel { name });
                self.engine.send_command(PluginCommand::GetState);
            }
            self.show_channel_dropdown = false;
        }
    }

    /// Handle `Message::AppRouteChanged { app_index, channel_index }`.
    pub(crate) fn handle_app_route_changed(&mut self, app_index: u32, channel_index: u32) {
        tracing::debug!(app_index, channel_index, "app route changed");
        self.engine.send_command(PluginCommand::RouteApp {
            app: app_index,
            channel: channel_index,
        });
    }

    /// Handle `Message::AppRoutingStarted(stream_index)`.
    pub(crate) fn handle_app_routing_started(&mut self, stream_index: u32) {
        tracing::debug!(
            stream_index,
            "app routing started — click a channel to assign"
        );
        self.routing_app = Some(stream_index);
    }

    /// Handle `Message::RefreshApps`.
    pub(crate) fn handle_refresh_apps(&mut self) {
        tracing::debug!("refresh apps requested");
        self.engine.send_command(PluginCommand::ListApplications);
    }
}
