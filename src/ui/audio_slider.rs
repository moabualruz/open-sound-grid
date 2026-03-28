use iced::widget::{column, slider, text};
use iced::{Element, Length};

use crate::app::Message;
use crate::ui::theme::TEXT_SECONDARY;

/// Compact horizontal volume slider with dB readout.
///
/// # Interaction
/// - **Drag**: Adjust volume smoothly
/// - **Scroll wheel**: Fine adjustment by ±1% (0.01 step)
/// - **Click bounds**: Jump to position
pub fn audio_slider<'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    let db_text = if value <= 0.001 {
        "-inf dB".to_string()
    } else {
        format!("{:.1} dB", 20.0 * value.log10())
    };

    const SCROLL_STEP: f32 = 0.01;
    tracing::trace!(
        value,
        db_display = %db_text,
        scroll_step = SCROLL_STEP,
        "audio_slider rendered with scroll wheel fine adjustment"
    );

    column![
        slider(0.0..=1.0, value, on_change)
            .step(SCROLL_STEP)
            .width(Length::Fill),
        text(db_text)
            .size(10)
            .color(TEXT_SECONDARY),
    ]
    .spacing(2)
    .width(Length::Fill)
    .into()
}
