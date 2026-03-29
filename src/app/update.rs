//! Message dispatch: routes all UI messages to their handlers.
//!
//! Large handler bodies live in `handlers/` submodules.
//! Small, self-contained arms stay inline for readability.

use iced::Task;

use crate::plugin::api::{PluginCommand, SourceId};
use crate::ui;

use super::messages::{ChannelPanelTab, Message};
use super::state::App;

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ── Matrix interactions (delegated) ─────────────────────────
            Message::RouteVolumeChanged {
                source,
                mix,
                volume,
            } => {
                self.handle_route_volume_changed(source, mix, volume);
            }
            Message::RouteToggled { source, mix } => {
                self.handle_route_toggled(source, mix);
            }
            Message::MixMasterVolumeChanged { mix, volume } => {
                tracing::debug!(?mix, volume, "mix master volume changed");
                self.engine.send_command(PluginCommand::SetMixMasterVolume { mix, volume });
            }
            Message::MixMuteToggled(mix) => {
                tracing::debug!(?mix, "mix mute toggled");
                let currently_muted = self.engine.state.mixes.iter()
                    .find(|m| m.id == mix).map_or(false, |m| m.muted);
                self.engine.send_command(PluginCommand::SetMixMuted { mix, muted: !currently_muted });
            }
            Message::SourceMuteToggled(source) => {
                let current_muted = match source {
                    SourceId::Channel(id) => self.engine.state.channels.iter()
                        .find(|c| c.id == id).map_or(false, |c| c.muted),
                    SourceId::Hardware(_) | SourceId::Mix(_) => false,
                };
                tracing::debug!(source = ?source, new_muted = !current_muted, "Toggling source mute");
                self.engine.send_command(PluginCommand::SetSourceMuted { source, muted: !current_muted });
            }
            Message::RouteMuteToggled { source, mix } => {
                let currently_muted = self.engine.state.routes
                    .get(&(source, mix)).map_or(false, |r| r.muted);
                tracing::debug!(?source, ?mix, new_muted = !currently_muted, "route mute toggled");
                self.engine.send_command(PluginCommand::SetRouteMuted { source, mix, muted: !currently_muted });
            }

            // ── App routing (delegated) ────────────────────────────────
            Message::AppRouteChanged { app_index, channel_index } => {
                self.handle_app_route_changed(app_index, channel_index);
            }
            Message::AppRoutingStarted(stream_index) => {
                self.handle_app_routing_started(stream_index);
            }
            Message::RefreshApps => {
                self.handle_refresh_apps();
            }

            // ── Channel picker ──────────────────────────────────────────
            Message::ToggleChannelPicker => {
                self.show_channel_picker = !self.show_channel_picker;
                tracing::debug!(show = self.show_channel_picker, "toggled channel type picker");
            }

            // ── Channel / Mix lifecycle (delegated) ─────────────────────
            Message::CreateChannel(name) => {
                self.handle_create_channel(name);
            }
            Message::CreateMix(name) => {
                self.handle_create_mix(name);
            }
            Message::StartRenameChannel(id) => {
                self.handle_start_rename_channel(id);
            }
            Message::StartRenameMix(id) => {
                self.handle_start_rename_mix(id);
            }
            Message::RenameInput(text) => {
                self.editing_text = text;
            }
            Message::ConfirmRename => {
                self.handle_confirm_rename();
            }
            Message::CancelRename => {
                self.handle_cancel_rename();
            }
            Message::RenameChannel { id, name } => {
                self.handle_rename_channel(id, name);
            }
            Message::RenameMix { id, name } => {
                self.handle_rename_mix(id, name);
            }
            Message::RemoveChannel(id) => {
                self.handle_remove_channel(id);
            }
            Message::RemoveMix(id) => {
                self.handle_remove_mix(id);
            }
            Message::MoveChannelUp(id) => {
                self.handle_move_channel_up(id);
            }
            Message::MoveChannelDown(id) => {
                self.handle_move_channel_down(id);
            }
            Message::UndoDelete => {
                self.handle_undo_delete();
            }
            Message::ClearUndo => {
                self.undo_buffer = None;
            }

            // ── Plugin events (delegated — largest handlers) ────────────
            Message::PluginStateRefreshed(snapshot) => {
                return self.handle_plugin_state_refreshed(snapshot);
            }
            Message::PluginDevicesChanged => {
                return self.handle_plugin_devices_changed();
            }
            Message::PluginAppsChanged(apps) => {
                return self.handle_plugin_apps_changed(apps);
            }
            Message::PluginPeakLevels(levels) => {
                self.handle_plugin_peak_levels(levels);
            }
            Message::PluginSpectrumData { channel, bins } => {
                self.handle_plugin_spectrum_data(channel, bins);
            }
            Message::PluginError(err) => {
                self.handle_plugin_error(err);
            }
            Message::PluginConnectionLost => {
                self.handle_plugin_connection_lost();
            }
            Message::PluginConnectionRestored => {
                self.handle_plugin_connection_restored();
            }

            // ── Tray & hotkeys (delegated) ──────────────────────────────
            Message::TrayShow => {
                return self.handle_tray_show();
            }
            Message::TrayQuit => {
                return self.handle_tray_quit();
            }
            Message::TrayMuteAll | Message::HotkeyMuteAll => {
                return self.handle_mute_all();
            }

            // ── Device selection (delegated) ────────────────────────────
            Message::MixOutputDeviceSelected { mix, device_name } => {
                return self.handle_mix_output_device_selected(mix, device_name);
            }

            // ── UI controls (inline — small) ────────────────────────────
            Message::SettingsToggled => {
                tracing::debug!(settings_open = !self.settings_open, "settings toggled");
                self.settings_open = !self.settings_open;
            }
            Message::WindowResized(width, height) => {
                tracing::trace!(width, height, "window resized");
                self.config.ui.window_width = width;
                self.config.ui.window_height = height;
            }
            Message::SidebarToggleCollapse => {
                tracing::debug!(collapsed = !self.sidebar_collapsed, "sidebar collapse toggled");
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.config.ui.compact_mode = self.sidebar_collapsed;
                if let Err(e) = self.config.save() {
                    tracing::error!(error = %e, "failed to save compact mode config");
                }
            }

            // ── Keyboard (delegated) ────────────────────────────────────
            Message::KeyPressed(key, modifiers) => {
                return self.handle_key_pressed(key, modifiers);
            }

            // ── Theme / Monitor ─────────────────────────────────────────
            Message::ThemeToggled => {
                let new_mode = match self.config.ui.theme_mode {
                    ui::theme::ThemeMode::Dark => ui::theme::ThemeMode::Light,
                    ui::theme::ThemeMode::Light => ui::theme::ThemeMode::System,
                    ui::theme::ThemeMode::System => ui::theme::ThemeMode::Dark,
                };
                tracing::info!(new_mode = ?new_mode, "theme toggled");
                self.config.ui.theme_mode = new_mode;
                let _ = self.config.save();
            }
            Message::MonitorMix(mix_id) => {
                tracing::info!(?mix_id, "monitor mix changed");
                self.monitored_mix = Some(mix_id);
            }

            // ── Channel dropdown / compact view ─────────────────────────
            Message::ToggleChannelDropdown => {
                self.show_channel_dropdown = !self.show_channel_dropdown;
                if self.show_channel_dropdown {
                    self.channel_search_text.clear();
                }
                tracing::debug!(open = self.show_channel_dropdown, "channel dropdown toggled");
            }
            Message::ChannelSearchInput(text) => {
                self.channel_search_text = text;
            }
            Message::CreateChannelFromApp(stream_index) => {
                self.handle_create_channel_from_app(stream_index);
            }
            Message::ToggleMixesView => {
                self.compact_mix_view = !self.compact_mix_view;
                tracing::debug!(compact = self.compact_mix_view, "mixes view toggled");
                if self.compact_mix_view {
                    self.compact_selected_mix = self.engine.state.mixes.first().map(|m| m.id);
                }
            }
            Message::SelectCompactMix(mix_id) => {
                tracing::debug!(mix_id = ?mix_id, "compact mix selected");
                self.compact_selected_mix = mix_id;
            }

            // ── Effects (delegated) ─────────────────────────────────────
            Message::CopyEffects(channel_id) => {
                return self.handle_copy_effects(channel_id);
            }
            Message::PasteEffects(channel_id) => {
                return self.handle_paste_effects(channel_id);
            }

            // ── Channel settings ────────────────────────────────────────
            Message::ChannelSettingsNameInput(text) => {
                tracing::debug!(text = %text, "channel settings name input");
                self.channel_settings_name = text;
            }
            Message::ChannelSettingsNameConfirm(channel_id) => {
                let name = self.channel_settings_name.clone();
                if !name.is_empty() {
                    tracing::info!(channel_id, name = %name, "renaming channel from settings panel");
                    self.engine.send_command(PluginCommand::RenameChannel {
                        id: channel_id, name: name.clone(),
                    });
                    if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == channel_id) {
                        if let Some(cfg) = self.config.channels.iter_mut().find(|c| c.name == ch.name) {
                            cfg.name = name;
                            let _ = self.config.save();
                        }
                    }
                }
            }

            // ── Presets (delegated) ─────────────────────────────────────
            Message::SavePreset(name) => {
                return self.handle_save_preset(name);
            }
            Message::LoadPreset(name) => {
                return self.handle_load_preset(name);
            }
            Message::PresetNameInput(text) => {
                return self.handle_preset_name_input(text);
            }

            // ── Channel master volume (delegated) ───────────────────────
            Message::ChannelMasterVolumeChanged { source, volume } => {
                self.handle_channel_master_volume_changed(source, volume);
            }

            // ── Audio settings ──────────────────────────────────────────
            Message::LatencyInput(text) => {
                if let Ok(ms) = text.parse::<u32>() {
                    let ms = ms.clamp(1, 500);
                    tracing::info!(latency_ms = ms, "latency changed");
                    self.config.audio.latency_ms = ms;
                    let _ = self.config.save();
                }
            }
            Message::ToggleStereoSliders => {
                self.config.ui.stereo_sliders = !self.config.ui.stereo_sliders;
                tracing::info!(stereo = self.config.ui.stereo_sliders, "stereo sliders toggled");
                let _ = self.config.save();
            }

            // ── Sound check ─────────────────────────────────────────────
            Message::SoundCheckStart => {
                tracing::info!("sound check: start recording");
                self.sound_check.start_recording();
            }
            Message::SoundCheckStop => {
                tracing::info!("sound check: stop recording");
                self.sound_check.stop_recording();
            }
            Message::SoundCheckPlayback => {
                tracing::info!("sound check: start playback");
                self.sound_check.start_playback();
            }
            Message::SoundCheckStopPlayback => {
                tracing::info!("sound check: stop playback");
                self.sound_check.stop_playback();
            }
            Message::SoundCheckSamples(samples) => {
                self.sound_check.append_samples(&samples);
            }

            // ── Channel selection / panel ────────────────────────────────
            Message::SelectedChannel(id) => {
                // If an app is pending routing, use this channel click to assign it.
                if let (Some(app_stream), Some(ch_id)) = (self.routing_app, id) {
                    if let Some(app_info) = self.engine.state.applications.iter()
                        .find(|a| a.stream_index == app_stream)
                    {
                        let binary = if app_info.binary.is_empty() {
                            app_info.name.clone()
                        } else {
                            app_info.binary.clone()
                        };
                        if let Some(ch_cfg) = self.config.channels.iter_mut().find(|c| {
                            self.engine.state.channels.iter()
                                .find(|ch| ch.id == ch_id)
                                .map(|ch| ch.name == c.name)
                                .unwrap_or(false)
                        }) {
                            if !ch_cfg.assigned_apps.contains(&binary) {
                                ch_cfg.assigned_apps.push(binary.clone());
                                let _ = self.config.save();
                            }
                        }
                        tracing::debug!(binary = %binary, channel_id = ch_id, "persisted assigned app binary");
                    }
                    tracing::info!(app_stream, channel_id = ch_id, "routing app to channel via two-step click");
                    self.engine.send_command(PluginCommand::RouteApp {
                        app: app_stream, channel: ch_id,
                    });
                    self.routing_app = None;
                    return Task::none();
                }
                tracing::debug!(channel_id = ?id, "selected channel for side panel");
                if self.selected_channel == id {
                    self.selected_channel = None;
                } else {
                    self.selected_channel = id;
                    self.channel_panel_tab = ChannelPanelTab::Apps;
                    if let Some(ch_id) = id {
                        if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == ch_id) {
                            self.channel_settings_name = ch.name.clone();
                        }
                    }
                }
            }
            Message::ChannelPanelTab(tab) => {
                self.channel_panel_tab = tab;
            }

            // ── App assign/unassign (delegated) ─────────────────────────
            Message::AssignApp { channel, stream_index } => {
                self.handle_assign_app(channel, stream_index);
            }
            Message::UnassignApp { channel, stream_index } => {
                self.handle_unassign_app(channel, stream_index);
            }

            // ── Effects toggle/params (delegated) ───────────────────────
            Message::EffectsToggled { channel, enabled } => {
                return self.handle_effects_toggled(channel, enabled);
            }
            Message::EffectsParamChanged { channel, param, value } => {
                return self.handle_effects_param_changed(channel, param, value);
            }
        }
        Task::none()
    }
}
