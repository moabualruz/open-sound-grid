//! Mix column header rendering.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Background, Border, Element, Length, Theme};
use lucide_icons::iced::{
    icon_headphones, icon_radio_tower, icon_users, icon_volume_2, icon_volume_x, icon_x,
};

use crate::app::Message;
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_elevated, bg_hover, border_color, text_muted, text_primary, text_secondary,
};

use super::{CELL_RADIUS, COL_WIDTH, HEADER_HEIGHT};

pub(super) fn mix_header<'a>(
    mix_id: u32,
    name: &str,
    color: iced::Color,
    has_output: bool,
    muted: bool,
    is_monitored: bool,
    theme_mode: ThemeMode,
    editing: bool,
    editing_text: &str,
    is_removable: bool,
    output_names: &[String],
    selected_output: Option<&str>,
) -> Element<'a, Message> {
    tracing::trace!(mix_id, name = %name, has_output, muted, is_monitored, editing, "rendering mix header");

    let color_bar = container(text("").size(1))
        .width(Length::Fill)
        .height(Length::Fixed(3.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        });

    let mute_icon = if muted {
        icon_volume_x().size(9).center()
    } else {
        icon_volume_2().size(9).center()
    };
    let mute_btn = button(mute_icon)
        .width(16)
        .height(16)
        .on_press(Message::MixMuteToggled(mix_id))
        .padding(0)
        .style(move |_: &Theme, _status| button::Style {
            background: if muted {
                Some(Background::Color(ACCENT))
            } else {
                None
            },
            text_color: text_primary(theme_mode),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

    let monitor_icon = icon_headphones().size(9).center();
    let monitor_btn = button(monitor_icon)
        .width(16)
        .height(16)
        .on_press(Message::MonitorMix(mix_id))
        .padding(0)
        .style(move |_: &Theme, _status| button::Style {
            background: if is_monitored {
                Some(Background::Color(ACCENT))
            } else {
                None
            },
            text_color: if is_monitored {
                text_primary(theme_mode)
            } else {
                text_muted(theme_mode)
            },
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

    tracing::trace!(mix_id, is_monitored, "rendering mix monitor button");

    let remove_btn = button(icon_x().size(10).color(text_muted(theme_mode)).center())
        .width(12)
        .height(12)
        .on_press_maybe(if is_removable {
            Some(Message::RemoveMix(mix_id))
        } else {
            None
        })
        .padding(0)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(Background::Color(bg_hover(theme_mode)))
                }
                _ => None,
            },
            text_color: text_muted(theme_mode),
            ..Default::default()
        });

    tracing::trace!(mix_id, "rendering mix remove button");

    // Per-mix output device dropdown (with "None" option to unassign)
    let mut out_options = vec!["None".to_string()];
    out_options.extend(output_names.iter().cloned());
    let sel_out = selected_output.map(|s| s.to_string());
    let mix_for_output = mix_id;
    let output_picker: Element<'a, Message> = pick_list(out_options, sel_out, move |name| {
        Message::MixOutputDeviceSelected {
            mix: mix_for_output,
            device_name: name,
        }
    })
    .placeholder("Select output...")
    .text_size(9)
    .width(Length::Fill)
    .into();

    // WL3 mix icon: map name prefix to icon
    let name_lower = name.to_lowercase();
    let mix_icon_el: Element<'a, Message> =
        if name_lower.contains("personal") || name_lower.contains("monitor") {
            icon_headphones()
                .size(16)
                .color(text_secondary(theme_mode))
                .center()
                .into()
        } else if name_lower.contains("chat") || name_lower.contains("voice") {
            icon_radio_tower()
                .size(16)
                .color(text_secondary(theme_mode))
                .center()
                .into()
        } else if name_lower.contains("stream") {
            icon_users()
                .size(16)
                .color(text_secondary(theme_mode))
                .center()
                .into()
        } else {
            icon_headphones()
                .size(16)
                .color(text_muted(theme_mode))
                .center()
                .into()
        };

    container(
        column![
            color_bar,
            row![
                mute_btn,
                monitor_btn,
                Space::new().width(Length::Fill),
                remove_btn,
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
            row![mix_icon_el, {
                let mix_name_el: Element<'a, Message> = if editing {
                    text_input("Name...", editing_text)
                        .on_input(Message::RenameInput)
                        .on_submit(Message::ConfirmRename)
                        .size(11)
                        .width(Length::Fixed(80.0))
                        .into()
                } else if is_removable {
                    // Renameable (all except Main/Monitor)
                    button(
                        text(name.to_string())
                            .size(12)
                            .color(text_primary(theme_mode))
                            .center(),
                    )
                    .on_press(Message::StartRenameMix(mix_id))
                    .padding(0)
                    .style(|_: &Theme, _| button::Style::default())
                    .into()
                } else {
                    // Main/Monitor: not renameable, just static text
                    text(name.to_string())
                        .size(12)
                        .color(text_primary(theme_mode))
                        .center()
                        .into()
                };
                mix_name_el
            },]
            .spacing(4)
            .align_y(iced::Alignment::Center),
            output_picker,
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fixed(COL_WIDTH))
    .height(Length::Fixed(HEADER_HEIGHT))
    .padding([4, 8])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg_elevated(theme_mode))),
        border: Border {
            color: border_color(theme_mode),
            width: 1.0,
            radius: CELL_RADIUS.into(),
        },
        ..Default::default()
    })
    .into()
}
