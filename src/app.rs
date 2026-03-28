use iced::widget::{column, container, row, rule, text, Space};
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

    // Plugin events
    PluginError(String),

    // UI
    SettingsToggled,
    Tick,
}

/// Application state.
pub struct App {
    pub config: AppConfig,
    pub engine: MixerEngine,
    pub settings_open: bool,
}

impl App {
    pub fn new() -> Self {
        let config = AppConfig::load();

        Self {
            config,
            engine: MixerEngine::new(),
            settings_open: false,
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RouteVolumeChanged { source, mix, volume } => {
                self.engine.send_command(PluginCommand::SetRouteVolume {
                    source,
                    mix,
                    volume,
                });
            }
            Message::RouteToggled { source, mix } => {
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
                self.engine
                    .send_command(PluginCommand::SetMixMasterVolume { mix, volume });
            }
            Message::MixMuteToggled(mix) => {
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
                self.engine.send_command(PluginCommand::SetSourceMuted {
                    source,
                    muted: true, // TODO: toggle
                });
            }
            Message::AppRouteChanged {
                app_index,
                channel_index,
            } => {
                self.engine.send_command(PluginCommand::RouteApp {
                    app: app_index,
                    channel: channel_index,
                });
            }
            Message::RefreshApps => {
                self.engine
                    .send_command(PluginCommand::ListApplications);
            }
            Message::PluginError(err) => {
                tracing::error!("Plugin error: {}", err);
            }
            Message::SettingsToggled => {
                self.settings_open = !self.settings_open;
            }
            Message::Tick => {
                // TODO: poll events from plugin bridge
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let header = container(
            row![
                text("OpenSoundGrid").size(20),
                Space::new().width(Length::Fill),
                text("Settings").size(14),
            ]
            .padding(8)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::BG_SECONDARY)),
            ..Default::default()
        });

        let matrix = ui::matrix::matrix_placeholder();

        let apps = ui::app_list::app_list_panel(&self.engine.state.applications);

        let status_text = if self.engine.is_connected() {
            "Connected"
        } else {
            "Disconnected"
        };

        let status_bar = container(
            row![
                text(status_text).size(11),
                Space::new().width(Length::Fill),
                text(format!("{} channels", self.config.channels.len())).size(11),
            ]
            .padding(4),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::BG_PRIMARY)),
            ..Default::default()
        });

        let content = column![header, rule::horizontal(1), matrix, rule::horizontal(1), apps, status_bar,]
            .spacing(0);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::BG_PRIMARY)),
                ..Default::default()
            })
            .into()
    }
}
