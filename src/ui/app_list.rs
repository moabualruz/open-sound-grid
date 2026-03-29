//! Panel showing detected audio applications with routing controls.
//!
//! Two-step click workflow:
//! 1. Click an app entry → app becomes "selected" (highlighted, emits AppRoutingStarted)
//! 2. Click a channel label in the matrix → app is routed there (SelectedChannel handler
//!    checks routing_app and calls RouteApp if set)
//! 3. Selection clears automatically after routing

use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::plugin::api::{AudioApplication, ChannelInfo};
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_elevated, bg_hover, border_color, text_muted, text_primary,
    text_secondary,
};

/// Panel showing detected audio applications.
///
/// Each app is a clickable button. Clicking an app selects it for routing
/// (highlights with accent background). Then clicking a channel label in the
/// matrix assigns the app to that channel.
pub fn app_list_panel<'a>(
    apps: &'a [AudioApplication],
    channels: &'a [ChannelInfo],
    theme_mode: ThemeMode,
    routing_app: Option<u32>,
) -> Element<'a, Message> {
    tracing::trace!(
        app_count = apps.len(),
        channel_count = channels.len(),
        routing_app = ?routing_app,
        "rendering app list panel"
    );

    let header = text("Applications")
        .size(12)
        .color(text_secondary(theme_mode));

    let content = if apps.is_empty() {
        column![
            header,
            Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
            text("No audio apps detected")
                .size(11)
                .color(text_muted(theme_mode)),
        ]
        .spacing(4)
    } else {
        let routing_hint = if routing_app.is_some() {
            text("Click a channel label to assign →")
                .size(10)
                .color(ACCENT)
        } else {
            text("Click an app to start routing")
                .size(10)
                .color(text_muted(theme_mode))
        };

        let mut col = column![header, routing_hint].spacing(4);
        for app in apps {
            let entry = app_entry(app, channels, theme_mode, routing_app);
            col = col.push(entry);
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

/// Single app entry as a clickable button.
///
/// When this app is the active routing selection (`routing_app == Some(stream_index)`),
/// the button is highlighted with the accent color and shows "Click a channel to assign..."
/// hint text. Otherwise it shows a subtle "Click to route" label.
fn app_entry<'a>(
    app: &'a AudioApplication,
    channels: &'a [ChannelInfo],
    theme_mode: ThemeMode,
    routing_app: Option<u32>,
) -> Element<'a, Message> {
    let stream_idx = app.stream_index;
    let is_routing = routing_app == Some(stream_idx);

    tracing::trace!(
        app_name = %app.name,
        stream_index = stream_idx,
        is_routing,
        "rendering app entry"
    );

    // Current channel assignment label (e.g. "Music, FX" or "unassigned")
    let assigned: Vec<&str> = channels
        .iter()
        .filter(|ch| ch.apps.contains(&stream_idx))
        .map(|ch| ch.name.as_str())
        .collect();
    let assignment_label = if assigned.is_empty() {
        "unassigned".to_owned()
    } else {
        assigned.join(", ")
    };

    let hint = if is_routing {
        text("Click a channel to assign...")
            .size(9)
            .color(text_primary(theme_mode))
    } else {
        text(format!("→ {assignment_label}"))
            .size(9)
            .color(text_muted(theme_mode))
    };

    let inner = row![
        text(&app.name).size(11).color(text_primary(theme_mode)),
        Space::new().width(Length::Fill),
        hint,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    button(inner)
        .on_press(Message::AppRoutingStarted(stream_idx))
        .style(move |_: &Theme, status| button::Style {
            background: if is_routing {
                Some(Background::Color(ACCENT))
            } else {
                match status {
                    button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                    _ => Some(Background::Color(bg_elevated(theme_mode))),
                }
            },
            text_color: text_secondary(theme_mode),
            border: Border {
                color: if is_routing {
                    ACCENT
                } else {
                    border_color(theme_mode)
                },
                width: if is_routing { 2.0 } else { 1.0 },
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .padding([6, 8])
        .width(Length::Fill)
        .into()
}
