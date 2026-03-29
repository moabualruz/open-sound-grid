//! Handlers for plugin events (PluginAppsChanged, PluginDevicesChanged, etc.).
//!
//! The large `handle_plugin_state_refreshed` handler lives in `state_refresh.rs`.

use std::collections::HashMap;

use iced::Task;

use crate::plugin::api::{AudioApplication, ChannelId, PluginCommand, SourceId};

use crate::app::messages::Message;
use crate::app::state::App;

impl App {
    pub(crate) fn handle_plugin_devices_changed(&mut self) -> Task<Message> {
        tracing::debug!("devices changed, requesting state");

        // Check which config-assigned output devices are no longer present in the
        // current (pre-refresh) snapshot. Log a warning for each missing device so
        // operators can see the event immediately, before the state refresh lands.
        for mix_cfg in &self.config.mixes {
            if let Some(device_name) = &mix_cfg.output_device {
                let still_present = self
                    .engine
                    .state
                    .hardware_outputs
                    .iter()
                    .any(|o| &o.name == device_name);
                if !still_present {
                    tracing::warn!(
                        mix = %mix_cfg.name,
                        device = %device_name,
                        "assigned output device no longer present — will attempt failover after state refresh"
                    );
                }
            }
        }

        // Refresh the failover list with devices that are still available now,
        // so the PluginStateRefreshed handler can pick a live backup device.
        // Append any newly-seen devices without disrupting existing priority order.
        if !self.engine.state.hardware_outputs.is_empty() {
            let current_names: Vec<String> = self
                .engine
                .state
                .hardware_outputs
                .iter()
                .map(|o| o.name.clone())
                .collect();
            for name in &current_names {
                if !self.config.failover.output_devices.contains(name) {
                    self.config.failover.output_devices.push(name.clone());
                }
            }
            tracing::info!(
                devices = ?self.config.failover.output_devices,
                "failover list updated on device change"
            );
            if let Err(e) = self.config.save() {
                tracing::error!(error = %e, "failed to save config after failover list update");
            }
        }

        self.engine.send_command(PluginCommand::GetState);

        // Notify user about device change via desktop notification
        crate::notifications::notify_device_change(
            "Audio Device Changed",
            "An audio device was connected or disconnected.",
        );

        Task::none()
    }

    pub(crate) fn handle_plugin_apps_changed(
        &mut self,
        mut apps: Vec<AudioApplication>,
    ) -> Task<Message> {
        tracing::debug!(count = apps.len(), "applications changed");
        // Resolve display names via desktop entries
        for app in &mut apps {
            let (display_name, icon_path) =
                self.app_resolver.resolve(&app.binary, Some(&app.name));
            tracing::debug!(
                binary = %app.binary,
                raw_name = %app.name,
                resolved_name = %display_name,
                has_icon = icon_path.is_some(),
                "resolved app display name and icon"
            );
            app.name = display_name;
            app.icon_path = icon_path;
        }

        // Track seen apps for persistent history (Journey 1, 8)
        let mut seen_changed = false;
        for app in &apps {
            if !self.config.seen_apps.contains(&app.binary) {
                tracing::info!(binary = %app.binary, name = %app.name, "new app seen — adding to persistent history");
                self.config.seen_apps.push(app.binary.clone());
                seen_changed = true;
            }
        }
        if seen_changed {
            tracing::debug!(
                count = self.config.seen_apps.len(),
                "seen_apps updated, saving config"
            );
            let _ = self.config.save();
        }

        // Auto-reassign apps to their previously assigned channels.
        // Match by binary name first, fall back to app name for apps that
        // don't set APPLICATION_PROCESS_BINARY (e.g., haruna via GStreamer).
        for app in &apps {
            if app.channel.is_none() {
                let match_key = if !app.binary.is_empty() {
                    &app.binary
                } else {
                    &app.name
                };
                if let Some(ch) = self.engine.state.channels.iter().find(|c| {
                    c.assigned_app_binaries.contains(match_key)
                        || c.assigned_app_binaries
                            .iter()
                            .any(|b| b.eq_ignore_ascii_case(match_key))
                }) {
                    tracing::info!(
                        match_key = %match_key,
                        channel = %ch.name,
                        stream_index = app.stream_index,
                        "auto-reassigning app to previously assigned channel"
                    );
                    self.engine.send_command(PluginCommand::RouteApp {
                        app: app.stream_index,
                        channel: ch.id,
                    });
                }
            }
        }

        // Auto-create solo channels for unassigned playing apps.
        // Uses the same CreateChannelFromApp flow which creates a channel,
        // assigns the app, and persists to config. These are real channels
        // with full volume control — they just can't be renamed or have
        // additional apps manually assigned while in "solo" mode.
        // Skip apps in suppressed_solo_apps (explicitly unassigned by user).
        {
            let solo_stream_indices: Vec<u32> = apps
                .iter()
                .filter(|app| {
                    if app.channel.is_some() || app.binary.is_empty() {
                        return false;
                    }
                    let match_key = if !app.binary.is_empty() {
                        &app.binary
                    } else {
                        &app.name
                    };
                    // Skip if assigned via config to any channel
                    let assigned = self.engine.state.channels.iter().any(|c| {
                        c.assigned_app_binaries.contains(match_key)
                            || c.assigned_app_binaries
                                .iter()
                                .any(|b| b.eq_ignore_ascii_case(match_key))
                    });
                    // Skip if channel with this name exists
                    let exists = self
                        .engine
                        .state
                        .channels
                        .iter()
                        .any(|c| c.name.eq_ignore_ascii_case(&app.name));
                    !assigned && !exists
                })
                .map(|app| app.stream_index)
                .collect();

            for stream_idx in solo_stream_indices {
                tracing::info!(stream_idx, "auto-creating solo channel for unassigned app");
                return self.update(Message::CreateChannelFromApp(stream_idx));
            }
        }

        // Also persist app name (not just binary) for display when not running
        for app in &apps {
            if !app.name.is_empty() && !app.binary.is_empty() {
                if !self.config.seen_apps.contains(&app.name) {
                    self.config.seen_apps.push(app.name.clone());
                }
            }
        }

        self.engine.state.update_applications(apps);

        Task::none()
    }

    pub(crate) fn handle_plugin_peak_levels(&mut self, levels: HashMap<SourceId, f32>) {
        tracing::trace!(count = levels.len(), "peak levels received");
        self.engine.state.update_peaks(levels);
    }

    pub(crate) fn handle_plugin_spectrum_data(
        &mut self,
        channel: ChannelId,
        bins: Vec<(f32, f32)>,
    ) {
        tracing::trace!(channel, bin_count = bins.len(), "spectrum data received");
        self.spectrum_data.insert(channel, bins);
    }

    pub(crate) fn handle_plugin_error(&mut self, err: String) {
        tracing::error!(error = %err, "plugin error");
    }

    pub(crate) fn handle_plugin_connection_lost(&mut self) {
        tracing::warn!("plugin connection lost");
        self.engine.state.connected = false;
    }

    pub(crate) fn handle_plugin_connection_restored(&mut self) {
        tracing::info!("plugin connection restored");
        self.engine.send_command(PluginCommand::GetState);
    }
}
