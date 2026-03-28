use std::sync::{Mutex, OnceLock};

use iced::widget::{button, column, container, pick_list, row, text, Space};
use iced::{Element, Length, Subscription, Task, Theme};
use tokio::sync::mpsc;

use crate::config::{AppConfig, ChannelConfig, MixConfig};
use crate::engine::MixerEngine;
use crate::plugin::api::{ChannelId, MixId, MixerSnapshot, PluginCommand, PluginEvent, SourceId};
use crate::resolve::AppResolver;
use crate::tray;
use crate::ui;

/// Global slot for the plugin event receiver.
/// Set once during boot, consumed once by the subscription stream.
static EVENT_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<PluginEvent>>>> = OnceLock::new();

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
    AppRouteChanged {
        app_index: u32,
        channel_index: u32,
    },
    #[allow(dead_code)]
    RefreshApps,

    // Channel/mix creation
    CreateChannel(String),
    CreateMix(String),

    // Channel/mix removal
    RemoveChannel(ChannelId),
    RemoveMix(MixId),

    // Plugin events (from async subscription — zero latency)
    PluginStateRefreshed(MixerSnapshot),
    PluginDevicesChanged,
    PluginAppsChanged(Vec<crate::plugin::api::AudioApplication>),
    PluginPeakLevels(std::collections::HashMap<SourceId, f32>),
    PluginError(String),
    PluginConnectionLost,
    PluginConnectionRestored,

    // Tray commands
    TrayShow,
    TrayQuit,
    TrayMuteAll,

    // Window events
    WindowResized(u32, u32),

    // Output device selection
    MixOutputDeviceSelected { mix: MixId, device_name: String },

    // UI
    SettingsToggled,
    SidebarToggleCollapse,
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
}

impl App {
    pub fn new() -> Self {
        tracing::info!("initializing App");
        let config = AppConfig::load();
        let app_resolver = AppResolver::new();

        let sidebar_collapsed = config.ui.compact_mode;
        tracing::debug!(compact_mode = config.ui.compact_mode, "applying compact_mode from config");

        Self {
            config,
            engine: MixerEngine::new(),
            app_resolver,
            settings_open: false,
            sidebar_collapsed,
            pending_output_restores: Vec::new(),
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
            self.engine
                .send_command(PluginCommand::CreateChannel { name: ch.name.clone() });
        }
        for mx in &self.config.mixes {
            tracing::debug!(name = %mx.name, "restoring mix from config");
            self.engine
                .send_command(PluginCommand::CreateMix { name: mx.name.clone() });
        }
        if !self.config.channels.is_empty() || !self.config.mixes.is_empty() {
            tracing::debug!(
                channels = self.config.channels.len(),
                mixes = self.config.mixes.len(),
                "config restore complete, requesting state refresh"
            );
            self.engine.send_command(PluginCommand::GetState);
        }

        // Output device restoration happens after the first StateRefreshed arrives
        // because mix IDs aren't known until the plugin creates them.
        self.pending_output_restores = self
            .config
            .mixes
            .iter()
            .filter_map(|m| m.output_device.as_ref().map(|d| (m.name.clone(), d.clone())))
            .collect();
        tracing::debug!(
            count = self.pending_output_restores.len(),
            "config restore: queued output devices for restoration after first state refresh"
        );
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RouteVolumeChanged { source, mix, volume } => {
                tracing::debug!(?source, ?mix, volume, "route volume changed");
                self.engine.send_command(PluginCommand::SetRouteVolume {
                    source,
                    mix,
                    volume,
                });
            }
            Message::RouteToggled { source, mix } => {
                tracing::debug!(?source, ?mix, "route toggled");
                let currently_enabled = self
                    .engine
                    .state
                    .routes
                    .get(&(source, mix))
                    .map_or(true, |r| r.enabled);
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
                    SourceId::Hardware(_) => false,
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
                tracing::debug!(?source, ?mix, new_muted = !currently_muted, "route mute toggled");
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
            Message::RefreshApps => {
                tracing::debug!("refresh apps requested");
                self.engine.send_command(PluginCommand::ListApplications);
            }
            Message::CreateChannel(name) => {
                tracing::debug!(name = %name, "creating channel");
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
            Message::RemoveChannel(id) => {
                tracing::info!(channel_id = id, "removing channel");
                self.engine.send_command(PluginCommand::RemoveChannel { id });
                self.engine.send_command(PluginCommand::GetState);
            }
            Message::RemoveMix(id) => {
                tracing::info!(mix_id = id, "removing mix");
                self.engine.send_command(PluginCommand::RemoveMix { id });
                self.engine.send_command(PluginCommand::GetState);
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
                    .map(|c| ChannelConfig { name: c.name.clone() })
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
                        }
                    })
                    .collect();

                self.engine.apply_snapshot(snapshot);

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
                                self.engine.send_command(PluginCommand::SetMixOutput {
                                    mix,
                                    output,
                                });
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

                // Only persist when the channel or mix list actually changed
                if new_channels != self.config.channels || new_mixes != self.config.mixes {
                    self.config.channels = new_channels;
                    self.config.mixes = new_mixes;
                    tracing::debug!(
                        channels = self.config.channels.len(),
                        mixes = self.config.mixes.len(),
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
                    let (display_name, _icon_path) = self.app_resolver.resolve(
                        &app.binary,
                        Some(&app.name),
                    );
                    tracing::debug!(
                        binary = %app.binary,
                        raw_name = %app.name,
                        resolved_name = %display_name,
                        "resolved app display name"
                    );
                    app.name = display_name;
                }
                self.engine.state.update_applications(apps);
            }
            Message::PluginPeakLevels(levels) => {
                tracing::trace!(count = levels.len(), "peak levels received");
                self.engine.state.update_peaks(levels);
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
            Message::MixOutputDeviceSelected { mix: mix_id, device_name } => {
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
                        self.engine.state.mixes.iter().any(|m| m.id == mix_id && m.name == c.name)
                    }) {
                        mix_config.output_device = Some(device_name.clone());
                        if let Err(e) = self.config.save() {
                            tracing::error!(error = %e, "failed to save output device config");
                        } else {
                            tracing::debug!(mix_id, device_name = %device_name, "output device persisted to config");
                        }
                    } else {
                        tracing::warn!(mix_id, "MixOutputDeviceSelected: no matching config entry for mix");
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
        }
        Task::none()
    }

    /// Async subscriptions: plugin events + tray commands + window events.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(plugin_event_stream),
            Subscription::run(tray_event_stream),
            iced::event::listen_with(|event, _status, _id| match event {
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    Some(Message::WindowResized(size.width as u32, size.height as u32))
                }
                _ => None,
            }),
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        tracing::trace!("rendering view");

        // Header — flush, same bg as sidebar for visual continuity
        let header_title = if self.engine.is_connected() {
            "OpenSoundGrid — PulseAudio"
        } else {
            "OpenSoundGrid"
        };
        let compact_btn = button(
            text("⊟").size(13).color(ui::theme::TEXT_SECONDARY),
        )
        .on_press(Message::SettingsToggled)
        .style(|_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(ui::theme::BG_HOVER)),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            text_color: ui::theme::TEXT_SECONDARY,
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
                text(header_title).size(18).color(ui::theme::TEXT_PRIMARY),
                Space::new().width(Length::Fill),
                device_picker,
                Space::new().width(Length::Fixed(8.0)),
                compact_btn,
            ]
            .padding([10, 16])
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::BG_SECONDARY)),
            ..Default::default()
        });

        // Thin separator line (1px, BORDER color)
        let sep = || {
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(|_: &Theme| container::Style {
                    background: Some(iced::Background::Color(ui::theme::BORDER)),
                    ..Default::default()
                })
        };

        let sidebar = ui::sidebar::sidebar(
            self.sidebar_collapsed,
            &self.engine.state.hardware_inputs,
        );

        let matrix = ui::matrix::matrix_grid(&self.engine.state);

        let app_panel = ui::app_list::app_list_panel(
            &self.engine.state.applications,
            &self.engine.state.channels,
        );

        let connected = self.engine.is_connected();
        let channel_count = self.engine.state.channels.len();
        let route_count = self.engine.state.routes.len();
        tracing::trace!(connected, channels = channel_count, routes = route_count, "rendering status bar");

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

        // Status bar — same bg as header/sidebar for visual frame
        let status_bar = container(
            row![
                status_dot,
                Space::new().width(Length::Fixed(6.0)),
                text(status_text).size(11).color(ui::theme::TEXT_SECONDARY),
                Space::new().width(Length::Fill),
                text(format!("{} channels  ·  {} routes  ·  {}ms latency", channel_count, route_count, self.config.audio.latency_ms))
                    .size(11)
                    .color(ui::theme::TEXT_MUTED),
            ]
            .padding([4, 16])
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::BG_SECONDARY)),
            ..Default::default()
        });

        // Settings overlay — shown when settings_open is true
        let settings_panel: Option<Element<'_, Message>> = if self.settings_open {
            tracing::trace!(settings_open = true, "rendering settings panel");
            let latency_text = format!("Latency: {}ms", self.config.audio.latency_ms);
            let panel = container(
                column![
                    text("Settings").size(14).color(ui::theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
                    text(latency_text).size(12).color(ui::theme::TEXT_SECONDARY),
                    text(format!("Config: ~/.config/open-sound-grid/")).size(11).color(ui::theme::TEXT_MUTED),
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    text(format!("Plugin: {}", if self.engine.is_connected() { "PulseAudio" } else { "None" }))
                        .size(12).color(ui::theme::TEXT_SECONDARY),
                    text(format!("Channels: {} / Mixes: {}", self.engine.state.channels.len(), self.engine.state.mixes.len()))
                        .size(12).color(ui::theme::TEXT_SECONDARY),
                ]
                .spacing(4)
            )
            .padding(12)
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::BG_ELEVATED)),
                border: iced::Border {
                    color: ui::theme::BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });
            Some(panel.into())
        } else {
            None
        };

        // Right panel: flush stack — header, sep, body, [settings], app panel, sep, status
        let mut right_panel = column![header, sep(), matrix, app_panel]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(settings) = settings_panel {
            right_panel = right_panel.push(settings);
        }

        let right_panel = right_panel.push(sep()).push(status_bar);

        let content = row![sidebar, right_panel];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::BG_PRIMARY)),
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
                        PluginEvent::ApplicationsChanged(apps) => {
                            Message::PluginAppsChanged(apps)
                        }
                        PluginEvent::PeakLevels(levels) => Message::PluginPeakLevels(levels),
                        PluginEvent::Error(err) => Message::PluginError(err),
                        PluginEvent::ConnectionLost => Message::PluginConnectionLost,
                        PluginEvent::ConnectionRestored => Message::PluginConnectionRestored,
                    };
                    if sender.try_send(msg).is_err() {
                        tracing::warn!("subscription sender full, dropping event");
                    }
                }
                None => {
                    tracing::warn!("plugin event channel closed");
                    let _ = sender
                        .try_send(Message::PluginError("Plugin disconnected".into()));
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
