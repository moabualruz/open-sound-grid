use iced::widget::{button, column, container, rule, text, Space};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::plugin::api::HardwareInput;
use crate::ui::theme::{ACCENT, BG_HOVER, BG_SECONDARY, BORDER, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY};

const EXPANDED_WIDTH: f32 = 200.0;
const COLLAPSED_WIDTH: f32 = 48.0;

pub fn sidebar<'a>(collapsed: bool, hardware_inputs: &'a [HardwareInput]) -> Element<'a, Message> {
    tracing::trace!(collapsed = collapsed, input_count = hardware_inputs.len(), "rendering sidebar");
    let width = if collapsed { COLLAPSED_WIDTH } else { EXPANDED_WIDTH };

    let content = if collapsed {
        collapsed_view()
    } else {
        expanded_view(hardware_inputs)
    };

    // Sidebar container with right border baked in (no separate rule widget)
    container(content)
        .width(width)
        .height(Length::Fill)
        .padding([12, 12])
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_SECONDARY)),
            border: Border {
                color: BORDER,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn expanded_view<'a>(hardware_inputs: &'a [HardwareInput]) -> Element<'a, Message> {
    tracing::trace!(input_count = hardware_inputs.len(), "rendering expanded sidebar");
    let collapse_btn = button(text("«").size(14).center())
        .width(32)
        .on_press(Message::SidebarToggleCollapse)
        .style(|_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: TEXT_SECONDARY,
            ..Default::default()
        });

    let header = text("DEVICES").size(11).color(TEXT_SECONDARY);

    let mut devices = column![header].spacing(4);
    for input in hardware_inputs {
        devices = devices.push(text(&input.name).size(13).color(TEXT_PRIMARY));
    }
    if hardware_inputs.is_empty() {
        devices = devices.push(text("No devices").size(12).color(TEXT_MUTED));
    }

    // Active mix item with left accent border
    let mix_item = container(text("Mixes").size(13).color(TEXT_PRIMARY))
        .padding([4, 8])
        .style(|_: &Theme| container::Style {
            border: Border {
                color: ACCENT,
                width: 2.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    let settings_btn = button(text("Settings").size(13).color(TEXT_SECONDARY))
        .on_press(Message::SettingsToggled)
        .padding([6, 8])
        .style(|_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: TEXT_SECONDARY,
            ..Default::default()
        });

    column![
        collapse_btn,
        devices,
        rule::horizontal(1).style(|_: &Theme| rule::Style {
            color: BORDER,
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

fn collapsed_view<'a>() -> Element<'a, Message> {
    tracing::trace!("rendering collapsed sidebar");
    let settings_btn = button(text("*").size(16).center())
        .width(32)
        .on_press(Message::SettingsToggled)
        .style(|_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: TEXT_SECONDARY,
            ..Default::default()
        });

    let expand_btn = button(text("»").size(14).center())
        .width(32)
        .on_press(Message::SidebarToggleCollapse)
        .style(|_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: TEXT_SECONDARY,
            ..Default::default()
        });

    column![
        expand_btn,
        text("#").size(16).color(TEXT_PRIMARY).center(),
        Space::new().width(Length::Fill).height(Length::Fill),
        settings_btn,
    ]
    .spacing(12)
    .align_x(iced::Alignment::Center)
    .height(Length::Fill)
    .into()
}
