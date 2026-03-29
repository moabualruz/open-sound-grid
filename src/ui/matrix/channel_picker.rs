//! Channel creation picker — preset buttons and custom name input.

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Background, Border, Element, Length, Theme};
use lucide_icons::iced::icon_plus;

use crate::app::Message;
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_elevated, bg_hover, border_color, text_muted, text_primary,
    text_secondary,
};

use super::{CELL_RADIUS, CHANNEL_PRESETS};

/// Build the channel picker dropdown (preset channels + custom name input)
/// or a simple "Create channel" button when the picker is hidden.
pub fn channel_picker<'a>(
    show: bool,
    channel_search: &str,
    existing_channel_names: &[String],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    if show {
        picker_dropdown(channel_search, existing_channel_names, theme_mode)
    } else {
        create_channel_button(theme_mode)
    }
}

fn picker_dropdown<'a>(
    channel_search: &str,
    existing_channel_names: &[String],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    let mut dropdown_col = column![].spacing(4);

    // Custom channel name input
    let search_input = text_input("Custom channel name...", channel_search)
        .on_input(Message::ChannelSearchInput)
        .on_submit(Message::CreateChannel(channel_search.to_string()))
        .size(12)
        .padding([4, 8]);
    dropdown_col = dropdown_col.push(search_input);

    // Empty channel presets — only show names NOT already in the channel list
    tracing::debug!(
        presets = CHANNEL_PRESETS.len(),
        "channel picker: rendering empty channel presets"
    );
    dropdown_col = dropdown_col.push(
        text("Add empty channel")
            .size(10)
            .color(text_muted(theme_mode)),
    );
    let mut preset_row = row![].spacing(4);
    for &(label, _tag) in CHANNEL_PRESETS {
        // Skip presets that already exist as channels (1 of each)
        if existing_channel_names.contains(&label.to_lowercase()) {
            continue;
        }
        let name = label.to_string();
        let btn = button(
            text(label)
                .size(10)
                .color(text_primary(theme_mode))
                .center(),
        )
        .on_press(Message::CreateChannel(name))
        .padding([3, 8])
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(ACCENT)),
                _ => Some(Background::Color(bg_hover(theme_mode))),
            },
            text_color: text_primary(theme_mode),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });
        preset_row = preset_row.push(btn);
    }
    dropdown_col = dropdown_col.push(scrollable(preset_row).direction(
        iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new(),
        ),
    ));

    container(dropdown_col)
        .padding([8, 8])
        .width(Length::Fill)
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

fn create_channel_button<'a>(theme_mode: ThemeMode) -> Element<'a, Message> {
    tracing::debug!("channel picker hidden: rendering create channel button");
    let add_btn = button(
        row![
            icon_plus().size(12).color(text_secondary(theme_mode)),
            text("Create channel")
                .size(12)
                .color(text_secondary(theme_mode)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::ToggleChannelPicker)
    .padding([6, 12])
    .style(move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
            _ => None,
        },
        text_color: text_secondary(theme_mode),
        border: Border {
            color: border_color(theme_mode),
            width: 1.0,
            radius: CELL_RADIUS.into(),
        },
        ..Default::default()
    });
    container(add_btn).padding([8, 0]).into()
}
