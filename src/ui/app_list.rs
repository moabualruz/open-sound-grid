//! Panel showing detected audio applications with routing controls.
//!
//! Each detected app can be assigned to a channel via a button.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::plugin::api::{AudioApplication, ChannelInfo};
use crate::ui::theme::{ACCENT, BG_ELEVATED, BG_HOVER, BORDER, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY};

/// Panel showing detected audio applications.
///
/// Each app has a button per channel to route it there.
pub fn app_list_panel<'a>(
    apps: &'a [AudioApplication],
    channels: &'a [ChannelInfo],
) -> Element<'a, Message> {
    tracing::trace!(app_count = apps.len(), channel_count = channels.len(), "rendering app list panel");

    let header = text("Applications").size(12).color(TEXT_SECONDARY);

    let content = if apps.is_empty() {
        column![
            header,
            Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
            text("No audio apps detected").size(11).color(TEXT_MUTED),
        ]
        .spacing(4)
    } else {
        let mut col = column![header].spacing(4);
        for app in apps {
            let app_row = app_entry(app, channels);
            col = col.push(app_row);
        }
        col
    };

    container(content)
        .padding(8)
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(BG_ELEVATED)),
            border: Border {
                color: BORDER,
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
) -> Element<'a, Message> {
    let name = text(&app.name).size(11).color(TEXT_PRIMARY);
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
                    (false, button::Status::Hovered) => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                text_color: if is_routed { TEXT_PRIMARY } else { TEXT_SECONDARY },
                border: Border {
                    color: BORDER,
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
