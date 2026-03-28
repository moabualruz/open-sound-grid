use std::sync::{Mutex, OnceLock};

use iced::widget::{column, container, row, text, Space};
use iced::{Element, Length, Subscription, Task, Theme};
use tokio::sync::mpsc;

use crate::config::{AppConfig, ChannelConfig, MixConfig};
use crate::engine::MixerEngine;
use crate::plugin::api::{MixId, MixerSnapshot, PluginCommand, PluginEvent, SourceId};
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

    // Source controls
    SourceMuteToggled(SourceId),

    // Application routing
    AppRouteChanged {
        app_index: u32,
        channel_index: u32,
    },
    RefreshApps,

    // Channel/mix creation
    CreateChannel(String),
    CreateMix(String),

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
}

impl App {
    pub fn new() -> Self {
        tracing::info!("initializing App");
        let config = AppConfig::load();
        let app_resolver = AppResolver::new();

        Self {
            config,
            engine: MixerEngine::new(),
            app_resolver,
            settings_open: false,
            sidebar_collapsed: false,
        }
    }

    /// Store the plugin event receiver for the subscription to consume.
    pub fn set_event_receiver(rx: mpsc::UnboundedReceiver<PluginEvent>) {
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
                    .map(|m| MixConfig {
                        name: m.name.clone(),
                        icon: String::new(),
                        color: [128, 128, 128],
                        output_device: None,
                    })
                    .collect();

                self.engine.apply_snapshot(snapshot);

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
                self.engine.state.update_peaks(levels);
            }
            Message::PluginError(err) => {
                tracing::error!(error = %err, "plugin error");
            }
            Message::PluginConnectionLost => {
                tracing::warn!("plugin connection lost");
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
                tracing::info!("tray: quit requested");
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
            Message::SettingsToggled => {
                tracing::debug!(settings_open = !self.settings_open, "settings toggled");
                self.settings_open = !self.settings_open;
            }
            Message::SidebarToggleCollapse => {
                tracing::debug!(
                    collapsed = !self.sidebar_collapsed,
                    "sidebar collapse toggled"
                );
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }
        }
        Task::none()
    }

    /// Async subscriptions: plugin events + tray commands.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(plugin_event_stream),
            Subscription::run(tray_event_stream),
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        tracing::trace!("rendering view");

        // Header — flush, same bg as sidebar for visual continuity
        let header = container(
            row![
                text("OpenSoundGrid").size(18).color(ui::theme::TEXT_PRIMARY),
                Space::new().width(Length::Fill),
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

        let status_text = if self.engine.is_connected() {
            "Connected"
        } else {
            "Disconnected"
        };

        // Status bar — same bg as header/sidebar for visual frame
        let status_bar = container(
            row![
                text(status_text).size(11).color(ui::theme::TEXT_SECONDARY),
                Space::new().width(Length::Fill),
                text(format!("{} channels", self.engine.state.channels.len()))
                    .size(11)
                    .color(ui::theme::TEXT_MUTED),
            ]
            .padding([4, 16]),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::BG_SECONDARY)),
            ..Default::default()
        });

        // Right panel: flush stack — header, sep, body, app panel, sep, status
        let right_panel = column![header, sep(), matrix, app_panel, sep(), status_bar,]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

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
                        tray::TrayCommand::Show => Message::TrayShow,
                        tray::TrayCommand::Hide => Message::TrayShow, // treat as show
                        tray::TrayCommand::Quit => Message::TrayQuit,
                        tray::TrayCommand::MuteAll => Message::TrayMuteAll,
                    };
                    if sender.try_send(msg).is_err() {
                        tracing::warn!("tray subscription sender full, dropping command");
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
