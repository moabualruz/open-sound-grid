use iced::widget::{column, container, text};
use iced::{Element, Length};

use crate::app::Message;

/// Placeholder for the matrix grid widget.
///
/// The matrix grid is the core UI component:
/// - Rows = audio sources (hardware inputs + software channels)
/// - Columns = output mixes
/// - Each intersection = volume fader + route toggle
///
/// v0.1 implementation will use iced Canvas for custom rendering.
pub fn matrix_placeholder<'a>() -> Element<'a, Message> {
    container(
        column![
            text("Matrix Mixer").size(18),
            text("Sources × Mixes routing grid").size(13),
        ]
        .spacing(8),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_theme: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(crate::ui::theme::BG_PANEL)),
        border: iced::Border {
            color: crate::ui::theme::BORDER_SUBTLE,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}
