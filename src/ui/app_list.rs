use iced::widget::{column, container, text};
use iced::{Element, Length};

use crate::app::Message;
use crate::audio::types::AudioApplication;

/// Panel showing detected audio applications.
///
/// Users drag/assign apps to channels from this list.
pub fn app_list_panel<'a>(apps: &[AudioApplication]) -> Element<'a, Message> {
    let content = if apps.is_empty() {
        column![
            text("Applications").size(14),
            text("No audio applications detected").size(12),
        ]
        .spacing(8)
    } else {
        let mut col = column![text("Applications").size(14)].spacing(4);
        for app in apps {
            col = col.push(text(format!("  {} (PID: {})", app.name, app.id.0)).size(12));
        }
        col
    };

    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(crate::ui::theme::BG_DARK)),
            border: iced::Border {
                color: crate::ui::theme::BORDER_SUBTLE,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}
