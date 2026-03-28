use iced::widget::{column, container, row, rule, text, Space};
use iced::{Element, Length, Task, Theme};

use crate::audio::types::{AudioApplication, MixId, MixerState, SourceId};
use crate::config::AppConfig;
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

    // Backend events
    BackendStateUpdate(MixerState),
    BackendError(String),

    // UI
    SettingsToggled,
    Tick,
}

/// Application state.
pub struct App {
    pub config: AppConfig,
    pub mixer: MixerState,
    pub applications: Vec<AudioApplication>,
    pub settings_open: bool,
}

impl App {
    pub fn new() -> Self {
        let config = AppConfig::load();

        Self {
            config,
            mixer: MixerState::default(),
            applications: Vec::new(),
            settings_open: false,
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RouteVolumeChanged { source, mix, volume } => {
                if let Some(route) = self.mixer.routes.get_mut(&(source, mix)) {
                    route.volume = volume;
                }
                // TODO: send BackendCommand::SetRouteVolume via bridge
            }
            Message::RouteToggled { source, mix } => {
                if let Some(route) = self.mixer.routes.get_mut(&(source, mix)) {
                    route.enabled = !route.enabled;
                }
            }
            Message::MixMasterVolumeChanged { mix, volume } => {
                if let Some(m) = self.mixer.mixes.iter_mut().find(|m| m.id == mix) {
                    m.master_volume = volume;
                }
            }
            Message::MixMuteToggled(mix) => {
                if let Some(m) = self.mixer.mixes.iter_mut().find(|m| m.id == mix) {
                    m.muted = !m.muted;
                }
            }
            Message::SourceMuteToggled(_source) => {
                // TODO: toggle mute for source across all mixes
            }
            Message::AppRouteChanged { .. } => {
                // TODO: route app to channel via bridge
            }
            Message::RefreshApps => {
                // TODO: request app list refresh via bridge
            }
            Message::BackendStateUpdate(state) => {
                self.mixer = state;
            }
            Message::BackendError(err) => {
                tracing::error!("Backend error: {}", err);
            }
            Message::SettingsToggled => {
                self.settings_open = !self.settings_open;
            }
            Message::Tick => {
                // TODO: poll peak levels from bridge
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
            background: Some(iced::Background::Color(ui::theme::BG_DARK)),
            ..Default::default()
        });

        let matrix = ui::matrix::matrix_placeholder();

        let apps = ui::app_list::app_list_panel(&self.applications);

        let status_bar = container(
            row![
                text("Ready").size(11),
                Space::new().width(Length::Fill),
                text(format!("{} channels", self.config.channels.len())).size(11),
            ]
            .padding(4),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Background::Color(ui::theme::BG_DARKEST)),
            ..Default::default()
        });

        let content = column![
            header,
            rule::horizontal(1),
            matrix,
            rule::horizontal(1),
            apps,
            status_bar,
        ]
        .spacing(0);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::BG_DARKEST)),
                ..Default::default()
            })
            .into()
    }
}
