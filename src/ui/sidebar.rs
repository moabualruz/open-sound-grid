use iced::widget::{Space, button, column, container, rule, text};
use iced::{Background, Border, Element, Length, Theme};
use lucide_icons::iced::{icon_chevron_left, icon_chevron_right, icon_headphones, icon_settings};

use crate::app::Message;
use crate::plugin::api::HardwareInput;
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_hover, bg_secondary, border_color, text_muted, text_primary,
    text_secondary,
};

const EXPANDED_WIDTH: f32 = 200.0;
const COLLAPSED_WIDTH: f32 = 48.0;

pub fn sidebar<'a>(
    collapsed: bool,
    hardware_inputs: &'a [HardwareInput],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    tracing::trace!(
        collapsed = collapsed,
        input_count = hardware_inputs.len(),
        "rendering sidebar"
    );
    let width = if collapsed {
        COLLAPSED_WIDTH
    } else {
        EXPANDED_WIDTH
    };

    let content = if collapsed {
        collapsed_view(theme_mode)
    } else {
        expanded_view(hardware_inputs, theme_mode)
    };

    // Sidebar container with right border baked in (no separate rule widget)
    container(content)
        .width(width)
        .height(Length::Fill)
        .padding([12, 12])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg_secondary(theme_mode))),
            border: Border {
                color: border_color(theme_mode),
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn expanded_view<'a>(
    hardware_inputs: &'a [HardwareInput],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    tracing::trace!(
        input_count = hardware_inputs.len(),
        "rendering expanded sidebar"
    );
    let collapse_btn = button(icon_chevron_left().size(14).center())
        .width(32)
        .on_press(Message::SidebarToggleCollapse)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_secondary(theme_mode),
            ..Default::default()
        });

    let header = text("DEVICES").size(11).color(text_secondary(theme_mode));

    let mut devices = column![header].spacing(4);
    for input in hardware_inputs {
        devices = devices.push(text(&input.name).size(13).color(text_primary(theme_mode)));
    }
    if hardware_inputs.is_empty() {
        devices = devices.push(text("No devices").size(12).color(text_muted(theme_mode)));
    }

    // Active mix item with left accent border
    let mix_item = container(text("Mixes").size(13).color(text_primary(theme_mode)))
        .padding([4, 8])
        .style(|_: &Theme| container::Style {
            border: Border {
                color: ACCENT,
                width: 2.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    let settings_btn = button(
        iced::widget::row![
            icon_settings().size(13).color(text_secondary(theme_mode)),
            text("Settings").size(13).color(text_secondary(theme_mode)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::SettingsToggled)
    .padding([6, 8])
    .style(move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
            _ => None,
        },
        text_color: text_secondary(theme_mode),
        ..Default::default()
    });

    column![
        collapse_btn,
        devices,
        rule::horizontal(1).style(move |_: &Theme| rule::Style {
            color: border_color(theme_mode),
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: true,
        }),
        mix_item,
        Space::new().width(Length::Fill).height(Length::Fill),
        settings_btn,
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

fn collapsed_view<'a>(theme_mode: ThemeMode) -> Element<'a, Message> {
    tracing::trace!("rendering collapsed sidebar");
    let settings_btn = button(icon_settings().size(16).center())
        .width(32)
        .on_press(Message::SettingsToggled)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_secondary(theme_mode),
            ..Default::default()
        });

    let expand_btn = button(icon_chevron_right().size(14).center())
        .width(32)
        .on_press(Message::SidebarToggleCollapse)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_secondary(theme_mode),
            ..Default::default()
        });

    column![
        expand_btn,
        icon_headphones()
            .size(16)
            .color(text_primary(theme_mode))
            .center(),
        Space::new().width(Length::Fill).height(Length::Fill),
        settings_btn,
    ]
    .spacing(12)
    .align_x(iced::Alignment::Center)
    .height(Length::Fill)
    .into()
}
