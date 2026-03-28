use iced::widget::{column, container, row, text, Space};
use iced::{Element, Length, Task, Theme};

use crate::config::AppConfig;
use crate::engine::MixerEngine;
use crate::plugin::api::{MixId, PluginCommand, SourceId};
use crate::ui;

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

    // Plugin events
    PluginError(String),

    // UI
    SettingsToggled,
    SidebarToggleCollapse,
    Tick,
}

/// Application state.
pub struct App {
    pub config: AppConfig,
    pub engine: MixerEngine,
    pub settings_open: bool,
    pub sidebar_collapsed: bool,
}

impl App {
    pub fn new() -> Self {
        tracing::info!("initializing App");
        let config = AppConfig::load();

        Self {
            config,
            engine: MixerEngine::new(),
            settings_open: false,
            sidebar_collapsed: false,
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
                tracing::debug!(?source, "source mute toggled");
                self.engine.send_command(PluginCommand::SetSourceMuted {
                    source,
                    muted: true, // TODO: toggle based on current state
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
                self.engine
                    .send_command(PluginCommand::ListApplications);
            }
            Message::CreateChannel(name) => {
                tracing::debug!(name = %name, "creating channel");
                self.engine
                    .send_command(PluginCommand::CreateChannel { name });
            }
            Message::CreateMix(name) => {
                tracing::debug!(name = %name, "creating mix");
                self.engine
                    .send_command(PluginCommand::CreateMix { name });
            }
            Message::PluginError(err) => {
                tracing::error!("Plugin error: {}", err);
            }
            Message::SettingsToggled => {
                tracing::debug!(settings_open = !self.settings_open, "settings toggled");
                self.settings_open = !self.settings_open;
            }
            Message::SidebarToggleCollapse => {
                tracing::debug!(collapsed = !self.sidebar_collapsed, "sidebar collapse toggled");
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }
            Message::Tick => {
                // TODO: poll events from plugin bridge
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
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

        // Right panel: flush stack — header, sep, body, sep, status
        let right_panel = column![header, sep(), matrix, sep(), status_bar,]
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
