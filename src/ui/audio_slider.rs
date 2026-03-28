use iced::widget::{column, container, slider, text};
use iced::Element;

use crate::app::Message;

/// Placeholder volume slider with dB label.
///
/// Will be replaced with a custom Canvas widget supporting:
/// - Vertical orientation
/// - dB-scaled response curve
/// - Fine adjustment on scroll
/// - Double-click to type exact value
pub fn volume_slider<'a>(
    label: &str,
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    let db_label = if value <= 0.0 {
        "-inf dB".to_string()
    } else {
        format!("{:.1} dB", 20.0 * value.log10())
    };

    container(
        column![
            text(label.to_string()).size(11),
            slider(0.0..=1.0, value, on_change).step(0.01),
            text(db_label).size(10),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center),
    )
    .padding(4)
    .into()
}
