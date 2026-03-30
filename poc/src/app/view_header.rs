//! Header bar: title, device picker, theme toggle, settings button.

use iced::widget::{Space, button, container, pick_list, row, text};
use iced::{Element, Length, Theme};
use lucide_icons::iced::{icon_expand, icon_moon, icon_settings, icon_shrink, icon_sun};

use crate::ui;

use super::messages::Message;
use super::state::App;

impl App {
    pub(crate) fn view_header(&self) -> Element<'_, Message> {
        let tm = self.config.ui.theme_mode;

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
            ui::theme::ThemeMode::System => icon_settings()
                .size(13)
                .color(ui::theme::text_secondary(tm)),
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

        container(
            row![
                text(header_title)
                    .size(18)
                    .color(ui::theme::text_primary(tm)),
                Space::new().width(Length::Fill),
                device_picker,
                Space::new().width(Length::Fixed(8.0)),
                // v0.4.0: Shrink/expand mixes toggle
                button(if self.compact_mix_view {
                    icon_expand().size(13).color(ui::theme::text_secondary(tm))
                } else {
                    icon_shrink().size(13).color(ui::theme::text_secondary(tm))
                },)
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
        })
        .into()
    }
}
