use iced::widget::{column, container, text};
use iced::Element;

use crate::app::Message;

/// Placeholder for the VU meter widget.
///
/// Will be a custom Canvas widget showing:
/// - Green/yellow/red gradient bar
/// - Peak hold indicator
/// - dB scale markings
pub fn vu_meter_placeholder<'a>(label: &str, _level: f32) -> Element<'a, Message> {
    container(
        column![
            text(label.to_string()).size(11),
            text("▮▮▮▮░░░░").size(11),
        ]
        .spacing(2),
    )
    .padding(4)
    .into()
}
