//! Handler for `PluginStateRefreshed` — config sync, deferred restore logic.

use iced::Task;

use crate::config::{ChannelConfig, MixConfig, RouteConfig};
use crate::plugin::api::{MixerSnapshot, PluginCommand, SourceId};

use crate::app::messages::Message;
use crate::app::state::App;

impl App {
    pub(crate) fn handle_plugin_state_refreshed(
        &mut self,
        snapshot: MixerSnapshot,
    ) -> Task<Message> {
        tracing::debug!(
            channels = snapshot.channels.len(),
            mixes = snapshot.mixes.len(),
            "state refreshed"
        );

        // Build new config lists from the snapshot before applying it
        let new_channels: Vec<ChannelConfig> = snapshot
            .channels
            .iter()
            .map(|c| {
                // Preserve assigned_apps from existing config
                let existing_apps = self
                    .config
                    .channels
                    .iter()
                    .find(|cfg| cfg.name == c.name)
                    .map(|cfg| cfg.assigned_apps.clone())
                    .unwrap_or_default();
                ChannelConfig {
                    name: c.name.clone(),
                    effects: c.effects.clone(),
                    muted: c.muted,
                    assigned_apps: existing_apps,
                    master_volume: self
                        .channel_master_volumes
                        .get(&c.id)
                        .copied()
                        .unwrap_or(1.0),
                }
            })
            .collect();
        let new_mixes: Vec<MixConfig> = snapshot
            .mixes
            .iter()
            .map(|m| {
                // Preserve existing icon/color/output_device from config
                let existing = self.config.mixes.iter().find(|c| c.name == m.name);
                // Resolve output device name from live engine state
                let live_output_name = m.output.and_then(|out_id| {
                    snapshot
                        .hardware_outputs
                        .iter()
                        .find(|o| o.id == out_id)
                        .map(|o| o.name.clone())
                });
                MixConfig {
                    name: m.name.clone(),
                    icon: existing.map(|c| c.icon.clone()).unwrap_or_default(),
                    color: existing.map(|c| c.color).unwrap_or([128, 128, 128]),
                    // Prefer live engine state; fall back to config
                    output_device: live_output_name
                        .or_else(|| existing.and_then(|c| c.output_device.clone())),
                    master_volume: m.master_volume,
                    muted: m.muted,
                }
            })
            .collect();

        // Pre-populate channel master volumes from config BEFORE apply_snapshot
        // so ratio computation inside apply_snapshot uses correct masters.
        for ch in &snapshot.channels {
            if !self.channel_master_volumes.contains_key(&ch.id) {
                if let Some(cfg) =
                    self.config.channels.iter().find(|c| c.name == ch.name)
                {
                    self.channel_master_volumes
                        .insert(ch.id, cfg.master_volume);
                    tracing::debug!(
                        channel = %ch.name,
                        master = cfg.master_volume,
                        "pre-restored channel master volume from config (before snapshot)"
                    );
                }
            }
        }

        self.engine
            .apply_snapshot(snapshot, &self.channel_master_volumes);

        // Populate assigned_app_binaries from config for not-running detection
        for ch in &mut self.engine.state.channels {
            if let Some(cfg) = self.config.channels.iter().find(|c| c.name == ch.name) {
                ch.assigned_app_binaries = cfg.assigned_apps.clone();
            }
        }

        // Auto-populate failover list on first boot when list is empty
        if self.config.failover.output_devices.is_empty()
            && !self.engine.state.hardware_outputs.is_empty()
        {
            self.config.failover.output_devices = self
                .engine
                .state
                .hardware_outputs
                .iter()
                .map(|o| o.name.clone())
                .collect();
            tracing::info!(
                devices = ?self.config.failover.output_devices,
                "auto-populated failover device list"
            );
            if let Err(e) = self.config.save() {
                tracing::error!(error = %e, "failed to save config after populating failover list");
            }
        }

        // Check for mixes whose output device has disappeared and attempt failover
        let lost_mixes: Vec<(u32, u32)> = self
            .engine
            .state
            .mixes
            .iter()
            .filter_map(|mix| {
                mix.output.and_then(|output_id| {
                    let exists = self
                        .engine
                        .state
                        .hardware_outputs
                        .iter()
                        .any(|o| o.id == output_id);
                    if exists {
                        None
                    } else {
                        Some((mix.id, output_id))
                    }
                })
            })
            .collect();

        for (mix_id, output_id) in lost_mixes {
            tracing::warn!(
                mix_id,
                output_id,
                "mix output device disappeared — attempting failover"
            );
            let fallback =
                self.config
                    .failover
                    .output_devices
                    .iter()
                    .find_map(|fallback_name| {
                        self.engine
                            .state
                            .hardware_outputs
                            .iter()
                            .find(|o| o.name == *fallback_name)
                            .map(|hw| (hw.id, hw.name.clone()))
                    });
            match fallback {
                Some((hw_id, hw_name)) => {
                    tracing::info!(
                        mix_id,
                        fallback = %hw_name,
                        "failover: switching to backup device"
                    );
                    self.engine.send_command(PluginCommand::SetMixOutput {
                        mix: mix_id,
                        output: hw_id,
                    });
                }
                None => {
                    tracing::warn!(
                        mix_id,
                        "failover: no available backup device found in failover list"
                    );
                }
            }
        }

        // Determine if state is "ready" — all expected channels and mixes
        // from config are present. Early snapshots during startup may be
        // incomplete (race with CreateChannel/CreateMix commands).
        let expected_channels = self.config.channels.len();
        let expected_mixes = self.config.mixes.len();
        let state_ready = self.engine.state.channels.len() >= expected_channels
            && self.engine.state.mixes.len() >= expected_mixes
            && expected_channels > 0
            && expected_mixes > 0;

        // Apply pending output, route, effects, and mix restores (see state_restore.rs)
        self.apply_deferred_restores(state_ready);

        // Auto-reassign ALL sink-inputs of apps to their previously assigned channels.
        // Apps arrive embedded in snapshot, not as PluginAppsChanged during startup.
        // Handles multi-stream apps (browser new tabs) by matching all unassigned
        // sink-inputs for the same binary.
        if state_ready {
            let mut reassigned = 0u32;
            for app in &self.engine.state.applications {
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
                            "auto-reassigning app on StateRefreshed"
                        );
                        self.engine.send_command(PluginCommand::RouteApp {
                            app: app.stream_index,
                            channel: ch.id,
                        });
                        reassigned += 1;
                    }
                }
            }
            if reassigned > 0 {
                tracing::info!(reassigned, "auto-reassigned apps on StateRefreshed");
            }

            // NOTE: We do NOT change the OS default sink.
            // The user's system default output stays untouched.
            // Apps are routed to channels explicitly via move-sink-input.

            // Auto-create solo channels for unassigned playing apps.
            // Skip apps in suppressed_solo_apps (explicitly unassigned by user).
            for app in &self.engine.state.applications {
                if app.channel.is_none() && !app.binary.is_empty() && !app.name.is_empty() {
                    let match_key = if !app.binary.is_empty() {
                        &app.binary
                    } else {
                        &app.name
                    };
                    let assigned = self.engine.state.channels.iter().any(|c| {
                        c.assigned_app_binaries.contains(match_key)
                            || c.assigned_app_binaries
                                .iter()
                                .any(|b| b.eq_ignore_ascii_case(match_key))
                    });
                    let exists = self
                        .engine
                        .state
                        .channels
                        .iter()
                        .any(|c| c.name.eq_ignore_ascii_case(&app.name));
                    if !assigned && !exists {
                        tracing::info!(
                            app = %app.name,
                            stream_index = app.stream_index,
                            "auto-creating solo channel for unassigned app (StateRefreshed)"
                        );
                        return self.update(Message::CreateChannelFromApp(app.stream_index));
                    }
                }
            }
        }

        // Rebuild route config from current engine state
        let new_routes: Vec<RouteConfig> = self
            .engine
            .state
            .routes
            .iter()
            .filter_map(|((source, mix_id), route)| {
                let channel_name = match source {
                    SourceId::Channel(id) => self
                        .engine
                        .state
                        .channels
                        .iter()
                        .find(|c| c.id == *id)
                        .map(|c| c.name.clone())?,
                    _ => return None,
                };
                let mix_name = self
                    .engine
                    .state
                    .mixes
                    .iter()
                    .find(|m| m.id == *mix_id)
                    .map(|m| m.name.clone())?;
                Some(RouteConfig {
                    channel_name,
                    mix_name,
                    volume: route.volume,
                    enabled: route.enabled,
                    muted: route.muted,
                    volume_left: route.volume_left,
                    volume_right: route.volume_right,
                })
            })
            .collect();

        // Only persist when lists actually changed AND the snapshot has real data.
        // During startup, early snapshots may have empty channels/mixes while the
        // plugin is still processing CreateChannel/CreateMix commands. Writing an
        // empty list to config would permanently lose the user's saved mixes.
        let snapshot_has_data = !new_channels.is_empty() || !new_mixes.is_empty();
        if snapshot_has_data
            && (new_channels != self.config.channels
                || new_mixes != self.config.mixes
                || new_routes != self.config.routes)
        {
            self.config.channels = new_channels;
            self.config.mixes = new_mixes;
            self.config.routes = new_routes;
            tracing::debug!(
                channels = self.config.channels.len(),
                mixes = self.config.mixes.len(),
                routes = self.config.routes.len(),
                "config changed, saving"
            );
            if let Err(e) = self.config.save() {
                tracing::error!(error = %e, "failed to save config");
            }
        }

        Task::none()
    }
}
