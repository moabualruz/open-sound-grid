use iced::widget::container;
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::ui::theme::{ThemeMode, VU_AMBER, VU_GREEN, VU_RED, bg_hover};

/// Horizontal VU meter: a colored bar on a rounded background.
///
/// * `level` - signal level clamped to 0.0..=1.0
/// * `width` - total meter width in pixels
/// * `height` - meter height in pixels
pub fn vu_meter(
    level: f32,
    width: f32,
    height: f32,
    theme_mode: ThemeMode,
) -> Element<'static, Message> {
    let level = level.clamp(0.0, 1.0);
    let fill_width = level * width;

    let fill_color = if level < 0.70 {
        VU_GREEN
    } else if level < 0.90 {
        VU_AMBER
    } else {
        VU_RED
    };

    let zone = if level < 0.70 {
        "green"
    } else if level < 0.90 {
        "amber"
    } else {
        "red"
    };
    tracing::trace!(level, zone, "vu_meter update");

    let bar = container(
        container("")
            .width(Length::Fixed(fill_width))
            .height(Length::Fixed(height))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill_color)),
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..Default::default()
            }),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg_hover(theme_mode))),
        border: Border {
            radius: 2.0.into(),
            ..Border::default()
        },
        ..Default::default()
    });

    bar.into()
}
