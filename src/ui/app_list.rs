//! Panel showing detected audio applications with routing controls.
//!
//! Each detected app can be assigned to a channel via a button.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::plugin::api::{AudioApplication, ChannelInfo};
use crate::ui::theme::{
    bg_elevated, bg_hover, border_color, text_muted, text_primary, text_secondary, ThemeMode,
    ACCENT,
};

/// Panel showing detected audio applications.
///
/// Each app has a button per channel to route it there.
pub fn app_list_panel<'a>(
    apps: &'a [AudioApplication],
    channels: &'a [ChannelInfo],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    tracing::trace!(app_count = apps.len(), channel_count = channels.len(), "rendering app list panel");

    let header = text("Applications").size(12).color(text_secondary(theme_mode));

    let content = if apps.is_empty() {
        column![
            header,
            Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
            text("No audio apps detected").size(11).color(text_muted(theme_mode)),
        ]
        .spacing(4)
    } else {
        let mut col = column![header].spacing(4);
        for app in apps {
            let app_row = app_entry(app, channels, theme_mode);
            col = col.push(app_row);
        }
        col
    };

    container(content)
        .padding(8)
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(bg_elevated(theme_mode))),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Single app entry with name and routing buttons.
fn app_entry<'a>(
    app: &'a AudioApplication,
    channels: &'a [ChannelInfo],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    tracing::trace!(app_name = %app.name, stream_index = app.stream_index, "rendering app entry");
    let name = text(&app.name).size(11).color(text_primary(theme_mode));
    let stream_idx = app.stream_index;

    let mut entry_row = row![name, Space::new().width(Length::Fill)].spacing(4)
        .align_y(iced::Alignment::Center);

    // One small route button per channel
    for channel in channels {
        let is_routed = channel.apps.contains(&stream_idx);
        let ch_id = channel.id;
        let label = if is_routed {
            format!("✓ {}", &channel.name)
        } else {
            channel.name.clone()
        };

        let btn = button(text(label).size(9).center())
            .padding([2, 6])
            .on_press(Message::AppRouteChanged {
                app_index: stream_idx,
                channel_index: ch_id,
            })
            .style(move |_: &Theme, status| button::Style {
                background: match (is_routed, status) {
                    (true, _) => Some(Background::Color(ACCENT)),
                    (false, button::Status::Hovered) => Some(Background::Color(bg_hover(theme_mode))),
                    _ => None,
                },
                text_color: if is_routed { text_primary(theme_mode) } else { text_secondary(theme_mode) },
                border: Border {
                    color: border_color(theme_mode),
                    width: 1.0,
                    radius: 2.0.into(),
                },
                ..Default::default()
            });

        entry_row = entry_row.push(btn);
    }

    container(entry_row)
        .padding([2, 4])
        .width(Length::Fill)
        .into()
}
