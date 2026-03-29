use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Border, Element, Length, Subscription, Task, Theme};
use lucide_icons::iced::{icon_expand, icon_moon, icon_settings, icon_shrink, icon_sun};
use tokio::sync::mpsc;

use crate::config::{AppConfig, ChannelConfig, MixConfig, RouteConfig};
use crate::effects::EffectsParams;
use crate::engine::MixerEngine;
use crate::plugin::api::{ChannelId, MixId, MixerSnapshot, PluginCommand, PluginEvent, SourceId};
use crate::resolve::AppResolver;
use crate::tray;
use crate::ui;

/// Global slot for the plugin event receiver.
/// Set once during boot, consumed once by the subscription stream.
static EVENT_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<PluginEvent>>>> = OnceLock::new();

/// Tab selection for the channel side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelPanelTab {
    Apps,
    Effects,
}

/// All possible UI messages.
#[derive(Debug, Clone)]
pub enum Message {
    // Matrix interactions
    RouteVolumeChanged {
        source: SourceId,
        mix: MixId,
        volume: f32,
    },
    RouteToggled {
        source: SourceId,
        mix: MixId,
    },

    // Mix controls
    MixMasterVolumeChanged {
        mix: MixId,
        volume: f32,
    },
    MixMuteToggled(MixId),

    // Source controls (mutes channel across ALL mixes)
    SourceMuteToggled(SourceId),

    // Route-level mute (mutes one source in one specific mix)
    RouteMuteToggled {
        source: SourceId,
        mix: MixId,
    },


    // Application routing
    /// Assign an app to a channel (checkbox checked in channel settings panel).
    AssignApp {
        channel: ChannelId,
        stream_index: u32,
    },
    /// Unassign an app from a channel (checkbox unchecked in channel settings panel).
    UnassignApp {
        channel: ChannelId,
        stream_index: u32,
    },
    /// Switch between Apps/Effects tabs in the channel side panel.
    ChannelPanelTab(ChannelPanelTab),
    #[allow(dead_code)]
    AppRouteChanged {
        app_index: u32,
        channel_index: u32,
    },
    /// User clicked an app entry to begin routing — next channel click will assign it.
    AppRoutingStarted(u32),
    #[allow(dead_code)]
    RefreshApps,

    // Channel/mix creation
    CreateChannel(String),
    CreateMix(String),
    /// Toggle the channel type picker visibility.
    ToggleChannelPicker,

    // Channel/mix removal (with undo support)
    RemoveChannel(ChannelId),
    RemoveMix(MixId),
    /// Move a channel up in the list.
    MoveChannelUp(ChannelId),
    /// Move a channel down in the list.
    MoveChannelDown(ChannelId),
    /// Undo the last delete operation.
    UndoDelete,
    /// Clear the undo buffer (called by timer).
    ClearUndo,

    // Inline rename (double-click)
    StartRenameChannel(ChannelId),
    StartRenameMix(MixId),
    RenameInput(String),
    ConfirmRename,
    CancelRename,
    RenameChannel {
        id: ChannelId,
        name: String,
    },
    RenameMix {
        id: MixId,
        name: String,
    },

    // Plugin events (from async subscription — zero latency)
    PluginStateRefreshed(MixerSnapshot),
    PluginDevicesChanged,
    PluginAppsChanged(Vec<crate::plugin::api::AudioApplication>),
    PluginPeakLevels(std::collections::HashMap<SourceId, f32>),
    /// FFT spectrum data received from plugin (future use).
    PluginSpectrumData {
        channel: ChannelId,
        bins: Vec<(f32, f32)>,
    },
    PluginError(String),
    PluginConnectionLost,
    PluginConnectionRestored,

    // Tray commands
    TrayShow,
    TrayQuit,
    TrayMuteAll,

    // Window events
    WindowResized(u32, u32),

    // Keyboard
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),

    // Output device selection
    MixOutputDeviceSelected {
        mix: MixId,
        device_name: String,
    },

    // Effects
    EffectsToggled {
        channel: ChannelId,
        enabled: bool,
    },
    EffectsParamChanged {
        channel: ChannelId,
        param: String,
        value: f32,
    },
    #[allow(dead_code)]
    SelectedChannel(Option<ChannelId>),

    // UI
    SettingsToggled,
    SidebarToggleCollapse,
    ThemeToggled,

    // v0.4.0: Channel creation dropdown
    /// Toggle the channel creation dropdown visibility.
    ToggleChannelDropdown,
    /// User typed in the channel creation search field.
    ChannelSearchInput(String),
    /// Create a channel directly from a detected app (by stream_index).
    CreateChannelFromApp(u32),

    // v0.4.0: Shrink/expand mixes view
    /// Toggle between full matrix and single-mix compact view.
    ToggleMixesView,
    /// In compact view, select which single mix to show.
    SelectCompactMix(Option<MixId>),

    // v0.4.0: Effects copy/paste
    /// Copy the selected channel's effects chain to clipboard.
    CopyEffects(ChannelId),
    /// Paste the copied effects chain to the selected channel.
    PasteEffects(ChannelId),

    // v0.4.0: Channel name editing in settings panel
    /// User edited the channel name in the settings panel text input.
    ChannelSettingsNameInput(String),
    /// Confirm channel name change from settings panel.
    ChannelSettingsNameConfirm(ChannelId),

    // Presets
    SavePreset(String),
    LoadPreset(String),
    PresetNameInput(String),
}

/// Application state.
pub struct App {
    pub config: AppConfig,
    pub engine: MixerEngine,
    pub app_resolver: AppResolver,
    pub settings_open: bool,
    pub sidebar_collapsed: bool,
    /// (mix_name, device_name) pairs waiting to be applied after first StateRefreshed.
    pub pending_output_restores: Vec<(String, String)>,
    /// Routes waiting to be replayed after next StateRefreshed (used by LoadPreset).
    pub pending_route_restores: Vec<RouteConfig>,
    /// (channel_name, effects, muted) restores deferred until after first StateRefreshed.
    pub pending_effects_restores: Vec<(String, EffectsParams, bool)>,
    /// (mix_name, master_volume, muted) restores deferred until after first StateRefreshed.
    pub pending_mix_restores: Vec<(String, f32, bool)>,
    /// Keyboard focus: channel row index.
    pub focused_row: Option<usize>,
    /// Keyboard focus: mix column index.
    pub focused_col: Option<usize>,
    /// Text input for preset name entry.
    pub preset_name_input: String,
    /// List of saved preset names (refreshed after save/load).
    pub available_presets: Vec<String>,
    /// Currently selected channel for effects panel display.
    pub selected_channel: Option<ChannelId>,
    /// Stream index of the app currently being routed (two-step click workflow).
    /// Set when the user clicks an app; cleared after they click a channel label.
    pub routing_app: Option<u32>,
    /// Per-channel FFT spectrum data (populated when SpectrumData plugin events arrive).
    pub spectrum_data: HashMap<ChannelId, Vec<(f32, f32)>>,
    /// Whether the channel type picker is visible.
    pub show_channel_picker: bool,
    /// Last deleted item for undo support (name, was_channel).
    /// Cleared after 10 seconds or after undo is triggered.
    pub undo_buffer: Option<(String, bool)>,
    /// Channel currently being renamed (inline edit mode).
    pub editing_channel: Option<ChannelId>,
    /// Mix currently being renamed (inline edit mode).
    pub editing_mix: Option<MixId>,
    /// Current text in the rename input field.
    pub editing_text: String,
    /// Active tab in the channel side panel (Apps or Effects).
    pub channel_panel_tab: ChannelPanelTab,
    /// v0.4.0: Whether the channel creation dropdown is open.
    pub show_channel_dropdown: bool,
    /// v0.4.0: Search text in the channel creation dropdown.
    pub channel_search_text: String,
    /// v0.4.0: Whether the matrix is in compact (shrunk) single-mix view.
    pub compact_mix_view: bool,
    /// v0.4.0: Which mix to show in compact view (None = all channels).
    pub compact_selected_mix: Option<MixId>,
    /// v0.4.0: Copied effects chain for paste between channels.
    pub copied_effects: Option<crate::effects::EffectsParams>,
    /// v0.4.0: Channel name text in the settings panel name field.
    pub channel_settings_name: String,
}

impl App {
    pub fn new() -> Self {
        tracing::info!("initializing App");
        let config = AppConfig::load();
        let app_resolver = AppResolver::new();

        let sidebar_collapsed = config.ui.compact_mode;
        tracing::debug!(
            compact_mode = config.ui.compact_mode,
            "applying compact_mode from config"
        );

        Self {
            config,
            engine: MixerEngine::new(),
            app_resolver,
            settings_open: false,
            sidebar_collapsed,
            pending_output_restores: Vec::new(),
            pending_route_restores: Vec::new(),
            pending_effects_restores: Vec::new(),
            pending_mix_restores: Vec::new(),
            focused_row: None,
            focused_col: None,
            preset_name_input: String::new(),
            available_presets: crate::presets::MixerPreset::list(),
            selected_channel: None,
            routing_app: None,
            spectrum_data: HashMap::new(),
            show_channel_picker: false,
            undo_buffer: None,
            editing_channel: None,
            editing_mix: None,
            editing_text: String::new(),
            channel_panel_tab: ChannelPanelTab::Apps,
            show_channel_dropdown: false,
            channel_search_text: String::new(),
            compact_mix_view: false,
            compact_selected_mix: None,
            copied_effects: None,
            channel_settings_name: String::new(),
        }
    }

    /// Store the plugin event receiver for the subscription to consume.
    pub fn set_event_receiver(rx: mpsc::UnboundedReceiver<PluginEvent>) {
        tracing::debug!("storing plugin event receiver in global slot");
        let _ = EVENT_RX.set(Mutex::new(Some(rx)));
    }

    /// Recreate channels and mixes from persisted config.
    /// Call after the plugin bridge is attached.
    pub fn restore_from_config(&mut self) {
        for ch in &self.config.channels {
            tracing::debug!(name = %ch.name, "restoring channel from config");
            self.engine.send_command(PluginCommand::CreateChannel {
                name: ch.name.clone(),
            });
        }
        for mx in &self.config.mixes {
            tracing::debug!(name = %mx.name, "restoring mix from config");
            self.engine.send_command(PluginCommand::CreateMix {
                name: mx.name.clone(),
            });
        }
        if !self.config.channels.is_empty() || !self.config.mixes.is_empty() {
            tracing::debug!(
                channels = self.config.channels.len(),
                mixes = self.config.mixes.len(),
                "config restore complete, requesting state refresh"
            );
            self.engine.send_command(PluginCommand::GetState);
        }

        // Output device, effects, and mix volume restores happen after the first
        // StateRefreshed arrives because channel/mix IDs aren't known until the
        // plugin creates them.
        self.pending_output_restores = self
            .config
            .mixes
            .iter()
            .filter_map(|m| {
                m.output_device
                    .as_ref()
                    .map(|d| (m.name.clone(), d.clone()))
            })
            .collect();
        tracing::debug!(
            count = self.pending_output_restores.len(),
            "config restore: queued output devices for restoration after first state refresh"
        );

        self.pending_effects_restores = self
            .config
            .channels
            .iter()
            .map(|c| (c.name.clone(), c.effects.clone(), c.muted))
            .collect();
        tracing::debug!(
            count = self.pending_effects_restores.len(),
            "config restore: queued effects params for restoration after first state refresh"
        );

        self.pending_mix_restores = self
            .config
            .mixes
            .iter()
            .map(|m| (m.name.clone(), m.master_volume, m.muted))
            .collect();
        tracing::debug!(
            count = self.pending_mix_restores.len(),
            "config restore: queued mix volumes for restoration after first state refresh"
        );
    }

    pub fn theme(&self) -> Theme {
        match self.config.ui.theme_mode {
            ui::theme::ThemeMode::Dark => Theme::Dark,
            ui::theme::ThemeMode::Light => Theme::Light,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RouteVolumeChanged {
                source,
                mix,
                volume,
            } => {
                tracing::debug!(?source, ?mix, volume, "route volume changed");
                self.engine.send_command(PluginCommand::SetRouteVolume {
                    source,
                    mix,
                    volume,
                });

                // Update ratio for linked slider behavior
                let master_vol = self
                    .engine
                    .state
                    .mixes
                    .iter()
                    .find(|m| m.id == mix)
                    .map_or(1.0, |m| m.master_volume);
                let new_ratio = if master_vol > 0.001 {
                    volume / master_vol
                } else {
                    1.0
                };
                self.engine
                    .state
                    .route_ratios
                    .insert((source, mix), new_ratio);
                tracing::debug!(?source, ?mix, new_ratio, "linked slider: ratio updated");
            }
            Message::RouteToggled { source, mix } => {
                tracing::debug!(?source, ?mix, "route toggled");
                let currently_enabled = self
                    .engine
                    .state
                    .routes
                    .get(&(source, mix))
                    .map_or(true, |r| r.enabled);
                if !currently_enabled {
                    // Route is being enabled (created); set initial ratio to 1.0
                    self.engine.state.route_ratios.insert((source, mix), 1.0);
                    tracing::debug!(
                        ?source,
                        ?mix,
                        "linked slider: new route ratio initialised to 1.0"
                    );
                } else {
                    // Route is being disabled (removed); clean up ratio entry
                    self.engine.state.route_ratios.remove(&(source, mix));
                    tracing::debug!(
                        ?source,
                        ?mix,
                        "linked slider: ratio removed for disabled route"
                    );
                }
                self.engine.send_command(PluginCommand::SetRouteEnabled {
                    source,
                    mix,
                    enabled: !currently_enabled,
                });
            }
            Message::MixMasterVolumeChanged { mix, volume } => {
                tracing::debug!(?mix, volume, "mix master volume changed");
                self.engine
                    .send_command(PluginCommand::SetMixMasterVolume { mix, volume });

                // Linked sliders: scale all channel faders in this mix proportionally
                let ratios: Vec<(SourceId, f32)> = self
                    .engine
                    .state
                    .route_ratios
                    .iter()
                    .filter(|((_, m), _)| *m == mix)
                    .map(|((s, _), ratio)| (*s, *ratio))
                    .collect();

                for (source, ratio) in ratios {
                    let new_vol = (volume * ratio).clamp(0.0, 1.0);
                    tracing::debug!(
                        ?source,
                        ?mix,
                        ratio,
                        new_vol,
                        "linked slider: scaling channel volume"
                    );
                    self.engine.send_command(PluginCommand::SetRouteVolume {
                        source,
                        mix,
                        volume: new_vol,
                    });
                }
            }
            Message::MixMuteToggled(mix) => {
                tracing::debug!(?mix, "mix mute toggled");
                let currently_muted = self
                    .engine
                    .state
                    .mixes
                    .iter()
                    .find(|m| m.id == mix)
                    .map_or(false, |m| m.muted);
                self.engine.send_command(PluginCommand::SetMixMuted {
                    mix,
                    muted: !currently_muted,
                });
            }
            Message::SourceMuteToggled(source) => {
                let current_muted = match source {
                    SourceId::Channel(id) => self
                        .engine
                        .state
                        .channels
                        .iter()
                        .find(|c| c.id == id)
                        .map_or(false, |c| c.muted),
                    SourceId::Hardware(_) | SourceId::Mix(_) => false,
                };
                tracing::debug!(source = ?source, new_muted = !current_muted, "Toggling source mute");
                self.engine.send_command(PluginCommand::SetSourceMuted {
                    source,
                    muted: !current_muted,
                });
            }
            Message::RouteMuteToggled { source, mix } => {
                let currently_muted = self
                    .engine
                    .state
                    .routes
                    .get(&(source, mix))
                    .map_or(false, |r| r.muted);
                tracing::debug!(
                    ?source,
                    ?mix,
                    new_muted = !currently_muted,
                    "route mute toggled"
                );
                self.engine.send_command(PluginCommand::SetRouteMuted {
                    source,
                    mix,
                    muted: !currently_muted,
                });
            }
            Message::AppRouteChanged {
                app_index,
                channel_index,
            } => {
                tracing::debug!(app_index, channel_index, "app route changed");
                self.engine.send_command(PluginCommand::RouteApp {
                    app: app_index,
                    channel: channel_index,
                });
            }
            Message::AppRoutingStarted(stream_index) => {
                tracing::debug!(
                    stream_index,
                    "app routing started — click a channel to assign"
                );
                self.routing_app = Some(stream_index);
            }
            Message::RefreshApps => {
                tracing::debug!("refresh apps requested");
                self.engine.send_command(PluginCommand::ListApplications);
            }
            Message::ToggleChannelPicker => {
                self.show_channel_picker = !self.show_channel_picker;
                tracing::debug!(
                    show = self.show_channel_picker,
                    "toggled channel type picker"
                );
            }
            Message::CreateChannel(name) => {
                tracing::debug!(name = %name, "creating channel");
                self.show_channel_picker = false;
                self.engine
                    .send_command(PluginCommand::CreateChannel { name });
                // Immediately request updated state
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::CreateMix(name) => {
                tracing::debug!(name = %name, "creating mix");
                self.engine.send_command(PluginCommand::CreateMix { name });
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::StartRenameChannel(id) => {
                tracing::debug!(channel_id = id, "starting channel rename");
                if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == id) {
                    self.editing_text = ch.name.clone();
                }
                self.editing_channel = Some(id);
                self.editing_mix = None;
            }
            Message::StartRenameMix(id) => {
                tracing::debug!(mix_id = id, "starting mix rename");
                if let Some(mx) = self.engine.state.mixes.iter().find(|m| m.id == id) {
                    self.editing_text = mx.name.clone();
                }
                self.editing_mix = Some(id);
                self.editing_channel = None;
            }
            Message::RenameInput(text) => {
                self.editing_text = text;
            }
            Message::ConfirmRename => {
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
            Message::CancelRename => {
                self.editing_channel = None;
                self.editing_mix = None;
                self.editing_text.clear();
            }
            Message::RenameChannel { id, name } => {
                tracing::info!(channel_id = id, name = %name, "renaming channel (direct)");
                self.engine
                    .send_command(PluginCommand::RenameChannel { id, name });
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::RenameMix { id, name } => {
                tracing::info!(mix_id = id, name = %name, "renaming mix (direct)");
                self.engine
                    .send_command(PluginCommand::RenameMix { id, name });
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::RemoveChannel(id) => {
                // Store name in undo buffer before deleting
                if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == id) {
                    self.undo_buffer = Some((ch.name.clone(), true));
                }
                tracing::info!(channel_id = id, "removing channel (undo available)");
                self.engine
                    .send_command(PluginCommand::RemoveChannel { id });
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::RemoveMix(id) => {
                if let Some(mx) = self.engine.state.mixes.iter().find(|m| m.id == id) {
                    self.undo_buffer = Some((mx.name.clone(), false));
                }
                tracing::info!(mix_id = id, "removing mix (undo available)");
                self.engine.send_command(PluginCommand::RemoveMix { id });
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::MoveChannelUp(id) => {
                if let Some(idx) = self.engine.state.channels.iter().position(|c| c.id == id) {
                    if idx > 0 {
                        self.engine.state.channels.swap(idx, idx - 1);
                        tracing::debug!(
                            channel_id = id,
                            from = idx,
                            to = idx - 1,
                            "moved channel up"
                        );
                    }
                }
            }
            Message::MoveChannelDown(id) => {
                if let Some(idx) = self.engine.state.channels.iter().position(|c| c.id == id) {
                    if idx + 1 < self.engine.state.channels.len() {
                        self.engine.state.channels.swap(idx, idx + 1);
                        tracing::debug!(
                            channel_id = id,
                            from = idx,
                            to = idx + 1,
                            "moved channel down"
                        );
                    }
                }
            }
            Message::UndoDelete => {
                if let Some((name, is_channel)) = self.undo_buffer.take() {
                    if is_channel {
                        tracing::info!(name = %name, "undoing channel deletion");
                        self.engine
                            .send_command(PluginCommand::CreateChannel { name });
                    } else {
                        tracing::info!(name = %name, "undoing mix deletion");
                        self.engine.send_command(PluginCommand::CreateMix { name });
                    }
                    self.engine.send_command(PluginCommand::GetState);
                }
            }
            Message::ClearUndo => {
                self.undo_buffer = None;
            }
            // Plugin events — arrive instantly via async subscription
            Message::PluginStateRefreshed(snapshot) => {
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
                        }
                    })
                    .collect();
                let new_mixes: Vec<MixConfig> = snapshot
                    .mixes
                    .iter()
                    .map(|m| {
                        // Preserve existing icon/color/output_device from config
                        let existing = self.config.mixes.iter().find(|c| c.name == m.name);
                        MixConfig {
                            name: m.name.clone(),
                            icon: existing.map(|c| c.icon.clone()).unwrap_or_default(),
                            color: existing.map(|c| c.color).unwrap_or([128, 128, 128]),
                            output_device: existing.and_then(|c| c.output_device.clone()),
                            master_volume: m.master_volume,
                            muted: m.muted,
                        }
                    })
                    .collect();

                self.engine.apply_snapshot(snapshot);

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

                // Apply any pending output device restores from config
                if !self.pending_output_restores.is_empty() {
                    let restores = std::mem::take(&mut self.pending_output_restores);
                    for (mix_name, device_name) in restores {
                        let mix_id = self
                            .engine
                            .state
                            .mixes
                            .iter()
                            .find(|m| m.name == mix_name)
                            .map(|m| m.id);
                        let hw_id = self
                            .engine
                            .state
                            .hardware_outputs
                            .iter()
                            .find(|o| o.name == device_name)
                            .map(|o| o.id);
                        match (mix_id, hw_id) {
                            (Some(mix), Some(output)) => {
                                tracing::info!(
                                    %mix_name,
                                    %device_name,
                                    mix_id = mix,
                                    output_id = output,
                                    "restoring output device from config"
                                );
                                self.engine
                                    .send_command(PluginCommand::SetMixOutput { mix, output });
                            }
                            _ => {
                                tracing::warn!(
                                    %mix_name,
                                    %device_name,
                                    mix_found = mix_id.is_some(),
                                    device_found = hw_id.is_some(),
                                    "could not restore output device — mix or device not found"
                                );
                            }
                        }
                    }
                }

                // Apply any pending route restores (from LoadPreset)
                if !self.pending_route_restores.is_empty() {
                    let restores = std::mem::take(&mut self.pending_route_restores);
                    tracing::info!(count = restores.len(), "applying pending route restores");
                    for route in restores {
                        let ch_id = self
                            .engine
                            .state
                            .channels
                            .iter()
                            .find(|c| c.name == route.channel_name)
                            .map(|c| c.id);
                        let mix_id = self
                            .engine
                            .state
                            .mixes
                            .iter()
                            .find(|m| m.name == route.mix_name)
                            .map(|m| m.id);
                        match (ch_id, mix_id) {
                            (Some(ch), Some(mix)) => {
                                let source = SourceId::Channel(ch);
                                tracing::debug!(
                                    channel = %route.channel_name,
                                    mix = %route.mix_name,
                                    volume = route.volume,
                                    enabled = route.enabled,
                                    muted = route.muted,
                                    "restoring route from preset"
                                );
                                if route.enabled {
                                    self.engine.send_command(PluginCommand::SetRouteEnabled {
                                        source,
                                        mix,
                                        enabled: true,
                                    });
                                    self.engine.send_command(PluginCommand::SetRouteVolume {
                                        source,
                                        mix,
                                        volume: route.volume,
                                    });
                                    if route.muted {
                                        self.engine.send_command(PluginCommand::SetRouteMuted {
                                            source,
                                            mix,
                                            muted: true,
                                        });
                                    }
                                }
                            }
                            _ => {
                                tracing::warn!(
                                    channel = %route.channel_name,
                                    mix = %route.mix_name,
                                    channel_found = ch_id.is_some(),
                                    mix_found = mix_id.is_some(),
                                    "pending route restore: channel or mix not found"
                                );
                            }
                        }
                    }
                }

                // Apply any pending effects/muted restores from config
                if !self.pending_effects_restores.is_empty() {
                    let restores = std::mem::take(&mut self.pending_effects_restores);
                    tracing::info!(count = restores.len(), "applying pending effects restores");
                    for (name, effects, muted) in restores {
                        if let Some(ch) = self.engine.state.channels.iter().find(|c| c.name == name)
                        {
                            if muted {
                                tracing::debug!(channel = %name, "restoring channel mute from config");
                                self.engine.send_command(PluginCommand::SetSourceMuted {
                                    source: SourceId::Channel(ch.id),
                                    muted: true,
                                });
                            }
                            if effects.enabled || effects != EffectsParams::default() {
                                tracing::debug!(
                                    channel = %name,
                                    enabled = effects.enabled,
                                    "restoring effects params from config"
                                );
                                self.engine.send_command(PluginCommand::SetEffectsParams {
                                    channel: ch.id,
                                    params: effects,
                                });
                            }
                        } else {
                            tracing::warn!(channel = %name, "pending effects restore: channel not found");
                        }
                    }
                }

                // Apply any pending mix volume/muted restores from config
                if !self.pending_mix_restores.is_empty() {
                    let restores = std::mem::take(&mut self.pending_mix_restores);
                    tracing::info!(
                        count = restores.len(),
                        "applying pending mix volume restores"
                    );
                    for (name, volume, muted) in restores {
                        if let Some(m) = self.engine.state.mixes.iter().find(|m| m.name == name) {
                            if (volume - 1.0).abs() > 0.001 {
                                tracing::debug!(mix = %name, volume, "restoring mix master volume from config");
                                self.engine.send_command(PluginCommand::SetMixMasterVolume {
                                    mix: m.id,
                                    volume,
                                });
                            }
                            if muted {
                                tracing::debug!(mix = %name, "restoring mix mute from config");
                                self.engine.send_command(PluginCommand::SetMixMuted {
                                    mix: m.id,
                                    muted: true,
                                });
                            }
                        } else {
                            tracing::warn!(mix = %name, "pending mix restore: mix not found");
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
                        })
                    })
                    .collect();

                // Only persist when channel, mix, or route lists actually changed
                if new_channels != self.config.channels
                    || new_mixes != self.config.mixes
                    || new_routes != self.config.routes
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
            }
            Message::PluginDevicesChanged => {
                tracing::debug!("devices changed, requesting state");
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::PluginAppsChanged(mut apps) => {
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

                // Apps appear as matrix rows directly — no null-sink creation needed.
                // Volume/mute controlled via set-sink-input-volume on the PA stream.
                self.engine.state.update_applications(apps);
            }
            Message::PluginPeakLevels(levels) => {
                tracing::trace!(count = levels.len(), "peak levels received");
                self.engine.state.update_peaks(levels);
            }
            Message::PluginSpectrumData { channel, bins } => {
                tracing::trace!(channel, bin_count = bins.len(), "spectrum data received");
                self.spectrum_data.insert(channel, bins);
            }
            Message::PluginError(err) => {
                tracing::error!(error = %err, "plugin error");
            }
            Message::PluginConnectionLost => {
                tracing::warn!("plugin connection lost");
                self.engine.state.connected = false;
            }
            Message::PluginConnectionRestored => {
                tracing::info!("plugin connection restored");
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::TrayShow => {
                tracing::info!("tray: show window requested");
                // iced doesn't have a show/hide window API in 0.14 —
                // the tray "Show" is a no-op for now (window is always visible)
            }
            Message::TrayQuit => {
                tracing::info!("tray: quit requested — saving config");
                let _ = self.config.save();
                return iced::exit();
            }
            Message::TrayMuteAll => {
                tracing::info!("tray: mute all requested");
                for channel in &self.engine.state.channels {
                    self.engine.send_command(PluginCommand::SetSourceMuted {
                        source: SourceId::Channel(channel.id),
                        muted: true,
                    });
                }
            }
            Message::MixOutputDeviceSelected {
                mix: mix_id,
                device_name,
            } => {
                tracing::debug!(mix_id, device_name = %device_name, "mix output device selected");
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
                            tracing::error!(error = %e, "failed to save output device config");
                        } else {
                            tracing::debug!(mix_id, device_name = %device_name, "output device persisted to config");
                        }
                    } else {
                        tracing::warn!(
                            mix_id,
                            "MixOutputDeviceSelected: no matching config entry for mix"
                        );
                    }
                } else {
                    tracing::warn!(device_name = %device_name, "MixOutputDeviceSelected: device not found in hardware_outputs");
                }
            }
            Message::SettingsToggled => {
                tracing::debug!(settings_open = !self.settings_open, "settings toggled");
                self.settings_open = !self.settings_open;
            }
            Message::WindowResized(width, height) => {
                tracing::trace!(width, height, "window resized");
                self.config.ui.window_width = width;
                self.config.ui.window_height = height;
                // Don't save on every resize event — too frequent. Config saved on quit.
            }
            Message::SidebarToggleCollapse => {
                tracing::debug!(
                    collapsed = !self.sidebar_collapsed,
                    "sidebar collapse toggled"
                );
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.config.ui.compact_mode = self.sidebar_collapsed;
                if let Err(e) = self.config.save() {
                    tracing::error!(error = %e, "failed to save compact mode config");
                }
            }
            Message::KeyPressed(key, modifiers) => {
                use iced::keyboard::Key;
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
                        tracing::debug!(
                            focused_row = ?self.focused_row,
                            focused_col = ?self.focused_col,
                            "keyboard: volume up"
                        );
                        if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                            if let (Some(ch), Some(mix)) = (
                                self.engine.state.channels.get(r),
                                self.engine.state.mixes.get(c),
                            ) {
                                let source = SourceId::Channel(ch.id);
                                let current_vol = self
                                    .engine
                                    .state
                                    .routes
                                    .get(&(source, mix.id))
                                    .map_or(1.0, |r| r.volume);
                                let new_vol = (current_vol + 0.01).min(1.0);
                                tracing::debug!(
                                    row = r,
                                    col = c,
                                    channel_id = ch.id,
                                    mix_id = mix.id,
                                    old_vol = current_vol,
                                    new_vol,
                                    "keyboard: volume up applied"
                                );
                                self.engine.send_command(PluginCommand::SetRouteVolume {
                                    source,
                                    mix: mix.id,
                                    volume: new_vol,
                                });
                            }
                        }
                    }
                    Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                        tracing::debug!(
                            focused_row = ?self.focused_row,
                            focused_col = ?self.focused_col,
                            "keyboard: volume down"
                        );
                        if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
                            if let (Some(ch), Some(mix)) = (
                                self.engine.state.channels.get(r),
                                self.engine.state.mixes.get(c),
                            ) {
                                let source = SourceId::Channel(ch.id);
                                let current_vol = self
                                    .engine
                                    .state
                                    .routes
                                    .get(&(source, mix.id))
                                    .map_or(1.0, |r| r.volume);
                                let new_vol = (current_vol - 0.01).max(0.0);
                                tracing::debug!(
                                    row = r,
                                    col = c,
                                    channel_id = ch.id,
                                    mix_id = mix.id,
                                    old_vol = current_vol,
                                    new_vol,
                                    "keyboard: volume down applied"
                                );
                                self.engine.send_command(PluginCommand::SetRouteVolume {
                                    source,
                                    mix: mix.id,
                                    volume: new_vol,
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
                    // v0.4.0: Ctrl+C/V for effects copy/paste
                    Key::Character(ref ch)
                        if (ch.as_str() == "c" || ch.as_str() == "C")
                            && modifiers.control() =>
                    {
                        if let Some(ch_id) = self.selected_channel {
                            tracing::info!(channel_id = ch_id, "keyboard: copy effects (Ctrl+C)");
                            return self.update(Message::CopyEffects(ch_id));
                        }
                    }
                    Key::Character(ref ch)
                        if (ch.as_str() == "v" || ch.as_str() == "V")
                            && modifiers.control() =>
                    {
                        if let Some(ch_id) = self.selected_channel {
                            tracing::info!(channel_id = ch_id, "keyboard: paste effects (Ctrl+V)");
                            return self.update(Message::PasteEffects(ch_id));
                        }
                    }
                    _ => {}
                }
            }
            Message::ThemeToggled => {
                let new_mode = match self.config.ui.theme_mode {
                    ui::theme::ThemeMode::Dark => ui::theme::ThemeMode::Light,
                    ui::theme::ThemeMode::Light => ui::theme::ThemeMode::Dark,
                };
                tracing::info!(new_mode = ?new_mode, "theme toggled");
                self.config.ui.theme_mode = new_mode;
                let _ = self.config.save();
            }
            // v0.4.0: Channel creation dropdown
            Message::ToggleChannelDropdown => {
                tracing::debug!(show = !self.show_channel_dropdown, "channel dropdown toggled");
                self.show_channel_dropdown = !self.show_channel_dropdown;
                if self.show_channel_dropdown {
                    self.channel_search_text.clear();
                    // Close old picker if open
                    self.show_channel_picker = false;
                }
            }
            Message::ChannelSearchInput(text) => {
                tracing::debug!(search = %text, "channel search input");
                self.channel_search_text = text;
            }
            Message::CreateChannelFromApp(stream_index) => {
                if let Some(app) = self
                    .engine
                    .state
                    .applications
                    .iter()
                    .find(|a| a.stream_index == stream_index)
                {
                    let name = app.name.clone();
                    tracing::info!(stream_index, name = %name, "creating channel from detected app");
                    self.engine
                        .send_command(PluginCommand::CreateChannel { name });
                    self.show_channel_dropdown = false;
                    // Route the app to the new channel after state refresh
                }
            }

            // v0.4.0: Shrink/expand mixes view
            Message::ToggleMixesView => {
                self.compact_mix_view = !self.compact_mix_view;
                tracing::debug!(compact = self.compact_mix_view, "mixes view toggled");
                if self.compact_mix_view {
                    // Default to first mix
                    self.compact_selected_mix = self.engine.state.mixes.first().map(|m| m.id);
                }
            }
            Message::SelectCompactMix(mix_id) => {
                tracing::debug!(mix_id = ?mix_id, "compact mix selected");
                self.compact_selected_mix = mix_id;
            }

            // v0.4.0: Effects copy/paste
            Message::CopyEffects(channel_id) => {
                if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == channel_id) {
                    tracing::info!(channel_id, name = %ch.name, "copied effects");
                    self.copied_effects = Some(ch.effects.clone());
                }
            }
            Message::PasteEffects(channel_id) => {
                if let Some(params) = self.copied_effects.clone() {
                    tracing::info!(channel_id, "pasting effects");
                    self.engine.send_command(PluginCommand::SetEffectsParams {
                        channel: channel_id,
                        params,
                    });
                }
            }

            // v0.4.0: Channel name editing in settings panel
            Message::ChannelSettingsNameInput(text) => {
                tracing::debug!(text = %text, "channel settings name input");
                self.channel_settings_name = text;
            }
            Message::ChannelSettingsNameConfirm(channel_id) => {
                let name = self.channel_settings_name.clone();
                if !name.is_empty() {
                    tracing::info!(channel_id, name = %name, "renaming channel from settings panel");
                    self.engine.send_command(PluginCommand::RenameChannel {
                        id: channel_id,
                        name: name.clone(),
                    });
                    // Update config
                    if let Some(ch) = self
                        .engine
                        .state
                        .channels
                        .iter()
                        .find(|c| c.id == channel_id)
                    {
                        if let Some(cfg) = self
                            .config
                            .channels
                            .iter_mut()
                            .find(|c| c.name == ch.name)
                        {
                            cfg.name = name;
                            let _ = self.config.save();
                        }
                    }
                }
            }

            Message::SavePreset(name) => {
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
            }
            Message::LoadPreset(name) => {
                tracing::info!(name = %name, "loading preset");
                match crate::presets::MixerPreset::load(&name) {
                    Ok(preset) => {
                        tracing::debug!(
                            channels = preset.channels.len(),
                            mixes = preset.mixes.len(),
                            routes = preset.routes.len(),
                            "preset loaded, queuing route restores"
                        );
                        self.config.channels = preset.channels;
                        self.config.mixes = preset.mixes;
                        // Queue route restores for deferred application after next StateRefreshed
                        self.pending_route_restores = preset
                            .routes
                            .iter()
                            .map(|r| RouteConfig {
                                channel_name: r.channel_name.clone(),
                                mix_name: r.mix_name.clone(),
                                volume: r.volume,
                                enabled: r.enabled,
                                muted: r.muted,
                            })
                            .collect();
                        tracing::debug!(
                            count = self.pending_route_restores.len(),
                            "route restores queued for next StateRefreshed"
                        );
                        let _ = self.config.save();
                        self.restore_from_config();
                    }
                    Err(e) => tracing::error!(error = %e, "failed to load preset"),
                }
            }
            Message::PresetNameInput(text) => {
                self.preset_name_input = text;
            }
            Message::SelectedChannel(id) => {
                // If an app is pending routing, use this channel click to assign it.
                if let (Some(app_stream), Some(ch_id)) = (self.routing_app, id) {
                    // Store the binary name for not-running app detection
                    if let Some(app_info) = self
                        .engine
                        .state
                        .applications
                        .iter()
                        .find(|a| a.stream_index == app_stream)
                    {
                        let binary = app_info.binary.clone();
                        // Persist in config
                        if let Some(ch_cfg) = self.config.channels.iter_mut().find(|c| {
                            self.engine
                                .state
                                .channels
                                .iter()
                                .find(|ch| ch.id == ch_id)
                                .map(|ch| ch.name == c.name)
                                .unwrap_or(false)
                        }) {
                            if !ch_cfg.assigned_apps.contains(&binary) {
                                ch_cfg.assigned_apps.push(binary.clone());
                                let _ = self.config.save();
                            }
                        }
                        tracing::debug!(
                            binary = %binary,
                            channel_id = ch_id,
                            "persisted assigned app binary"
                        );
                    }

                    tracing::info!(
                        app_stream,
                        channel_id = ch_id,
                        "routing app to channel via two-step click"
                    );
                    self.engine.send_command(PluginCommand::RouteApp {
                        app: app_stream,
                        channel: ch_id,
                    });
                    self.routing_app = None;
                    return Task::none();
                }
                tracing::debug!(channel_id = ?id, "selected channel for side panel");
                if self.selected_channel == id {
                    // Clicking same channel again closes the panel
                    self.selected_channel = None;
                } else {
                    self.selected_channel = id;
                    self.channel_panel_tab = ChannelPanelTab::Apps;
                    // Pre-fill channel name in settings panel
                    if let Some(ch_id) = id {
                        if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == ch_id)
                        {
                            self.channel_settings_name = ch.name.clone();
                        }
                    }
                }
            }
            Message::ChannelPanelTab(tab) => {
                tracing::debug!(tab = ?tab, "channel panel tab switched");
                self.channel_panel_tab = tab;
            }
            Message::AssignApp {
                channel,
                stream_index,
            } => {
                tracing::info!(stream_index, channel_id = channel, "assigning app to channel");
                // Persist binary name in config
                if let Some(app_info) = self
                    .engine
                    .state
                    .applications
                    .iter()
                    .find(|a| a.stream_index == stream_index)
                {
                    let binary = app_info.binary.clone();
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
                            let _ = self.config.save();
                        }
                    }
                }
                self.engine.send_command(PluginCommand::RouteApp {
                    app: stream_index,
                    channel,
                });
            }
            Message::UnassignApp {
                channel,
                stream_index,
            } => {
                tracing::info!(stream_index, channel_id = channel, "unassigning app from channel");
                // Remove binary from config
                if let Some(app_info) = self
                    .engine
                    .state
                    .applications
                    .iter()
                    .find(|a| a.stream_index == stream_index)
                {
                    let binary = &app_info.binary;
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
                self.engine.send_command(PluginCommand::UnrouteApp {
                    app: stream_index,
                });
            }
            Message::EffectsToggled { channel, enabled } => {
                tracing::debug!(channel_id = channel, enabled, "effects toggled");
                self.engine
                    .send_command(PluginCommand::SetEffectsEnabled { channel, enabled });
            }
            Message::EffectsParamChanged {
                channel,
                param,
                value,
            } => {
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
            }
        }
        Task::none()
    }

    /// Async subscriptions: plugin events + tray commands + window events + keyboard.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(plugin_event_stream),
            Subscription::run(tray_event_stream),
            iced::event::listen_with(|event, _status, _id| match event {
                iced::Event::Window(iced::window::Event::Resized(size)) => Some(
                    Message::WindowResized(size.width as u32, size.height as u32),
                ),
                _ => None,
            }),
            iced::keyboard::listen().filter_map(|event| match event {
                iced::keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    Some(Message::KeyPressed(key, modifiers))
                }
                _ => None,
            }),
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        tracing::trace!("rendering view");

        let tm = self.config.ui.theme_mode;

        // Header — flush, same bg as sidebar for visual continuity
        let header_title = if self.engine.is_connected() {
            "Open Sound Grid — PulseAudio"
        } else {
            "Open Sound Grid"
        };
        let compact_btn = button(
            icon_settings()
                .size(13)
                .color(ui::theme::text_secondary(tm)),
        )
        .on_press(Message::SettingsToggled)
        .style(move |_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(ui::theme::bg_hover(tm))),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            text_color: ui::theme::text_secondary(tm),
            ..Default::default()
        })
        .padding([2, 8]);

        let theme_icon_widget = match self.config.ui.theme_mode {
            ui::theme::ThemeMode::Dark => icon_sun().size(13).color(ui::theme::text_secondary(tm)),
            ui::theme::ThemeMode::Light => {
                icon_moon().size(13).color(ui::theme::text_secondary(tm))
            }
        };
        let theme_btn = button(theme_icon_widget)
            .on_press(Message::ThemeToggled)
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(iced::Background::Color(ui::theme::bg_hover(tm))),
                border: iced::Border {
                    radius: iced::border::Radius::from(4.0),
                    ..Default::default()
                },
                text_color: ui::theme::text_secondary(tm),
                ..Default::default()
            })
            .padding([2, 8]);

        let output_names: Vec<String> = self
            .engine
            .state
            .hardware_outputs
            .iter()
            .map(|o| o.name.clone())
            .collect();
        let selected_output = self
            .engine
            .state
            .mixes
            .first()
            .and_then(|m| m.output)
            .and_then(|out_id| {
                self.engine
                    .state
                    .hardware_outputs
                    .iter()
                    .find(|o| o.id == out_id)
            })
            .map(|o| o.name.clone());
        let first_mix_id = self.engine.state.mixes.first().map(|m| m.id).unwrap_or(0);
        let device_picker = pick_list(output_names, selected_output, move |name| {
            Message::MixOutputDeviceSelected {
                mix: first_mix_id,
                device_name: name,
            }
        })
        .placeholder("Select output...")
        .text_size(12);

        let header = container(
            row![
                text(header_title)
                    .size(18)
                    .color(ui::theme::text_primary(tm)),
                Space::new().width(Length::Fill),
                device_picker,
                Space::new().width(Length::Fixed(8.0)),
                // v0.4.0: Shrink/expand mixes toggle
                button(
                    if self.compact_mix_view {
                        icon_expand().size(13).color(ui::theme::text_secondary(tm))
                    } else {
                        icon_shrink().size(13).color(ui::theme::text_secondary(tm))
                    },
                )
                .on_press(Message::ToggleMixesView)
                .style(move |_theme: &Theme, _status| button::Style {
                    background: Some(iced::Background::Color(ui::theme::bg_hover(tm))),
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..Default::default()
                    },
                    text_color: ui::theme::text_secondary(tm),
                    ..Default::default()
                })
                .padding([2, 8]),
                Space::new().width(Length::Fixed(4.0)),
                theme_btn,
                Space::new().width(Length::Fixed(4.0)),
                compact_btn,
            ]
            .padding([10, 16])
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::bg_secondary(tm))),
            ..Default::default()
        });

        // Thin separator line (1px, BORDER color)
        let sep = move || {
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_: &Theme| container::Style {
                    background: Some(iced::Background::Color(ui::theme::border_color(tm))),
                    ..Default::default()
                })
        };

        let sidebar = ui::sidebar::sidebar(
            self.sidebar_collapsed,
            &self.engine.state.hardware_inputs,
            tm,
        );

        let matrix = ui::matrix::matrix_grid(
            &self.engine.state,
            self.focused_row,
            self.focused_col,
            tm,
            self.show_channel_picker,
            self.editing_channel,
            self.editing_mix,
            &self.editing_text,
            self.compact_mix_view,
            self.compact_selected_mix,
            &self.channel_search_text,
        );

        // App panel removed — apps auto-create channels or are managed inline
        // let app_panel = ui::app_list::app_list_panel(...);

        let connected = self.engine.is_connected();
        let channel_count = self.engine.state.channels.len();
        let route_count = self.engine.state.routes.len();
        tracing::trace!(
            connected,
            channels = channel_count,
            routes = route_count,
            "rendering status bar"
        );

        let (status_dot_color, status_text) = if connected {
            (ui::theme::STATUS_CONNECTED, "Connected")
        } else {
            (ui::theme::STATUS_ERROR, "Disconnected")
        };
        let status_dot = container(Space::new())
            .width(Length::Fixed(8.0))
            .height(Length::Fixed(8.0))
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(status_dot_color)),
                border: iced::Border {
                    radius: iced::border::Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        // Undo button (shown when undo_buffer has content)
        let undo_element: Option<Element<'_, Message>> =
            self.undo_buffer.as_ref().map(|(name, is_ch)| {
                let label = if *is_ch {
                    format!("Undo delete '{name}'")
                } else {
                    format!("Undo delete '{name}'")
                };
                button(text(label).size(10).color(ui::theme::text_primary(tm)))
                    .on_press(Message::UndoDelete)
                    .padding([2, 8])
                    .style(move |_: &Theme, status| button::Style {
                        background: match status {
                            button::Status::Hovered => {
                                Some(iced::Background::Color(ui::theme::ACCENT))
                            }
                            _ => Some(iced::Background::Color(ui::theme::bg_hover(tm))),
                        },
                        text_color: ui::theme::text_primary(tm),
                        border: Border {
                            color: ui::theme::border_color(tm),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            });

        // Status bar — same bg as header/sidebar for visual frame
        let mut status_row = row![
            status_dot,
            Space::new().width(Length::Fixed(6.0)),
            text(status_text)
                .size(11)
                .color(ui::theme::text_secondary(tm)),
        ]
        .align_y(iced::Alignment::Center);

        if let Some(undo) = undo_element {
            status_row = status_row
                .push(Space::new().width(Length::Fixed(12.0)))
                .push(undo);
        }

        status_row = status_row.push(Space::new().width(Length::Fill)).push(
            text(format!(
                "{} channels  ·  {} routes  ·  {}ms latency",
                channel_count, route_count, self.config.audio.latency_ms
            ))
            .size(11)
            .color(ui::theme::text_muted(tm)),
        );

        let status_bar = container(status_row.padding([4, 16]))
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::bg_secondary(tm))),
                ..Default::default()
            });

        // Settings overlay — shown when settings_open is true
        let settings_panel: Option<Element<'_, Message>> = if self.settings_open {
            tracing::trace!(settings_open = true, "rendering settings panel");
            let latency_text = format!("Latency: {}ms", self.config.audio.latency_ms);

            // Preset save row: text input + "Save" button
            let preset_name_val = self.preset_name_input.clone();
            let save_btn = button(text("Save").size(12))
                .on_press(Message::SavePreset(self.preset_name_input.clone()))
                .padding([2, 8]);
            let preset_save_row = row![
                text_input("Preset name…", &preset_name_val)
                    .on_input(Message::PresetNameInput)
                    .size(12)
                    .padding([2, 6]),
                Space::new().width(Length::Fixed(6.0)),
                save_btn,
            ]
            .align_y(iced::Alignment::Center);

            // Preset load row: pick_list + "Load" button
            let selected_preset: Option<String> = None;
            let preset_names = self.available_presets.clone();
            let load_btn = button(text("Load").size(12))
                .on_press_maybe(
                    selected_preset
                        .as_ref()
                        .map(|n| Message::LoadPreset(n.clone())),
                )
                .padding([2, 8]);
            let preset_load_row = row![
                pick_list(preset_names, selected_preset, Message::LoadPreset)
                    .placeholder("Select preset…")
                    .text_size(12),
                Space::new().width(Length::Fixed(6.0)),
                load_btn,
            ]
            .align_y(iced::Alignment::Center);

            let panel = container(
                column![
                    text("Settings").size(14).color(ui::theme::text_primary(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
                    text(latency_text)
                        .size(12)
                        .color(ui::theme::text_secondary(tm)),
                    text(format!("Config: ~/.config/open-sound-grid/"))
                        .size(11)
                        .color(ui::theme::text_muted(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    text(format!(
                        "Plugin: {}",
                        if self.engine.is_connected() {
                            "PulseAudio"
                        } else {
                            "None"
                        }
                    ))
                    .size(12)
                    .color(ui::theme::text_secondary(tm)),
                    text(format!(
                        "Channels: {} / Mixes: {}",
                        self.engine.state.channels.len(),
                        self.engine.state.mixes.len()
                    ))
                    .size(12)
                    .color(ui::theme::text_secondary(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
                    text("Presets").size(13).color(ui::theme::text_primary(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    preset_save_row,
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    preset_load_row,
                ]
                .spacing(4),
            )
            .padding(12)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::bg_elevated(tm))),
                border: iced::Border {
                    color: ui::theme::border_color(tm),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });
            Some(panel.into())
        } else {
            None
        };

        // Build the matrix area: matrix on the left, optional channel settings panel on the right
        let matrix_area: Element<'_, Message> = if let Some(ch_id) = self.selected_channel {
            if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == ch_id) {
                // Compute not-running binaries for this channel
                let running_binaries: Vec<&str> = self
                    .engine
                    .state
                    .applications
                    .iter()
                    .map(|a| a.binary.as_str())
                    .collect();
                let not_running: Vec<String> = ch
                    .assigned_app_binaries
                    .iter()
                    .filter(|b| !running_binaries.contains(&b.as_str()))
                    .cloned()
                    .collect();

                tracing::trace!(channel_id = ch_id, "rendering channel settings side panel");
                let side_panel = scrollable(
                    ui::channel_settings::channel_settings_panel(
                        ch,
                        &self.engine.state.applications,
                        not_running,
                        self.channel_panel_tab,
                        tm,
                        &self.channel_settings_name,
                    ),
                )
                .width(Length::Fixed(280.0))
                .height(Length::Fill);

                row![matrix, side_panel,]
                    .spacing(0)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                matrix
            }
        } else {
            matrix
        };

        // Right panel: flush stack — header, sep, matrix+effects, [settings], app panel, sep, status
        let mut right_panel = column![header, sep(), matrix_area]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        // Settings panel before app panel
        if let Some(settings) = settings_panel {
            right_panel = right_panel.push(settings);
        }

        // App panel removed — apps auto-create channels

        let right_panel = right_panel.push(sep()).push(status_bar);

        let content = row![sidebar, right_panel];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::bg_primary(tm))),
                border: iced::Border {
                    radius: iced::border::Radius {
                        top_left: 0.0,
                        top_right: 0.0,
                        bottom_right: 8.0,
                        bottom_left: 8.0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}

// --- Async plugin event stream (zero latency, no polling) ---

/// Produces a stream of Messages from the plugin event channel.
/// Called by `Subscription::run` — must be a `fn()` pointer.
fn plugin_event_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(64, async move |mut sender| {
        // Take the receiver from the global slot (consumed once)
        let mut rx = EVENT_RX
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|mut guard| guard.take());

        if rx.is_none() {
            tracing::warn!("no plugin event receiver — subscription idle");
            std::future::pending::<()>().await;
            return;
        }

        let rx = rx.as_mut().unwrap();
        tracing::info!("plugin event subscription started");

        loop {
            match rx.recv().await {
                Some(event) => {
                    let msg = match event {
                        PluginEvent::StateRefreshed(snapshot) => {
                            tracing::info!(
                                hardware_inputs = snapshot.hardware_inputs.len(),
                                hardware_outputs = snapshot.hardware_outputs.len(),
                                channels = snapshot.channels.len(),
                                "subscription received StateRefreshed"
                            );
                            Message::PluginStateRefreshed(snapshot)
                        }
                        PluginEvent::DevicesChanged => Message::PluginDevicesChanged,
                        PluginEvent::ApplicationsChanged(apps) => Message::PluginAppsChanged(apps),
                        PluginEvent::PeakLevels(levels) => Message::PluginPeakLevels(levels),
                        PluginEvent::Error(err) => Message::PluginError(err),
                        PluginEvent::ConnectionLost => Message::PluginConnectionLost,
                        PluginEvent::ConnectionRestored => Message::PluginConnectionRestored,
                        PluginEvent::SpectrumData { channel, bins } => {
                            Message::PluginSpectrumData { channel, bins }
                        }
                    };
                    if sender.try_send(msg).is_err() {
                        tracing::warn!("subscription sender full, dropping event");
                    }
                }
                None => {
                    tracing::warn!("plugin event channel closed");
                    let _ = sender.try_send(Message::PluginError("Plugin disconnected".into()));
                    std::future::pending::<()>().await;
                }
            }
        }
    })
}

// --- Tray command stream ---

/// Produces a stream of Messages from the tray command channel.
fn tray_event_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(16, async move |mut sender| {
        let mut rx = tray::TRAY_RX
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|mut guard| guard.take());

        if rx.is_none() {
            tracing::debug!("no tray command receiver — tray subscription idle");
            std::future::pending::<()>().await;
            return;
        }

        let rx = rx.as_mut().unwrap();
        tracing::info!("tray command subscription started");

        loop {
            match rx.recv().await {
                Some(cmd) => {
                    tracing::debug!(cmd = ?cmd, "tray command received");
                    let msg = match cmd {
                        tray::TrayCommand::Show => Some(Message::TrayShow),
                        tray::TrayCommand::Hide => {
                            tracing::debug!("tray hide — no-op (iced has no hide API)");
                            None
                        }
                        tray::TrayCommand::Quit => Some(Message::TrayQuit),
                        tray::TrayCommand::MuteAll => Some(Message::TrayMuteAll),
                    };
                    if let Some(msg) = msg {
                        if sender.try_send(msg).is_err() {
                            tracing::warn!("tray subscription sender full, dropping command");
                        }
                    }
                }
                None => {
                    tracing::debug!("tray command channel closed");
                    std::future::pending::<()>().await;
                }
            }
        }
    })
}
