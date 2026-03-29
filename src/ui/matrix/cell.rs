//! Route intersection cell rendering.

use iced::widget::{button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use lucide_icons::iced::{icon_plus, icon_volume_2, icon_volume_x};

use crate::app::Message;
use crate::plugin::api::SourceId;
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_elevated, bg_hover, border_color, text_muted, text_primary,
};

use super::{CELL_HEIGHT, CELL_HEIGHT_STEREO, CELL_RADIUS, COL_WIDTH};

pub(super) fn matrix_cell<'a>(
    source: SourceId,
    mix_id: u32,
    route: Option<&'a crate::plugin::api::RouteState>,
    cell_ratio: f32, // WL3: the cell's own percentage (0.0-1.0), NOT the effective PA volume
    channel_master: f32, // Channel master volume (0.0-1.0)
    peak: f32,
    focused: bool,
    theme_mode: ThemeMode,
    stereo_sliders: bool,
) -> Element<'a, Message> {
    tracing::trace!(source = ?source, mix_id, has_route = route.is_some(), cell_ratio, channel_master, peak, focused, "rendering matrix cell");
    let cell_content: Element<'a, Message> = match route {
        Some(route) => {
            let vol = cell_ratio; // slider position = ratio (what user controls)
            let effective = (cell_ratio * channel_master).clamp(0.0, 1.0); // actual output level
            let muted = route.muted;

            let mute_icon = if muted {
                icon_volume_x().size(9).center()
            } else {
                icon_volume_2().size(9).center()
            };
            let mute_btn = button(mute_icon)
                .width(16)
                .height(16)
                .on_press(Message::RouteMuteToggled {
                    source,
                    mix: mix_id,
                })
                .padding(0)
                .style(move |_: &Theme, _status| button::Style {
                    background: if muted {
                        Some(Background::Color(ACCENT))
                    } else {
                        None
                    },
                    text_color: text_primary(theme_mode),
                    border: Border {
                        color: border_color(theme_mode),
                        width: 1.0,
                        radius: 2.0.into(),
                    },
                    ..Default::default()
                });

            let src = source;
            let mid = mix_id;
            let vol_pct = (vol * 100.0).round() as u32;
            let eff_pct = (effective * 100.0).round() as u32;

            if stereo_sliders {
                // L/R mode: two sliders stacked, labeled L and R
                let src_l = source;
                let mid_l = mix_id;
                let src_r = source;
                let mid_r = mix_id;
                let slider_l = row![
                    text("L").size(8).color(text_muted(theme_mode)),
                    iced::widget::slider(0.0..=1.0_f32, vol, move |v| {
                        Message::RouteVolumeChanged {
                            source: src_l,
                            mix: mid_l,
                            volume: v,
                        }
                    })
                    .step(0.01)
                    .width(Length::Fill),
                ]
                .spacing(2)
                .align_y(iced::Alignment::Center);

                let slider_r = row![
                    text("R").size(8).color(text_muted(theme_mode)),
                    iced::widget::slider(0.0..=1.0_f32, vol, move |v| {
                        Message::RouteVolumeChanged {
                            source: src_r,
                            mix: mid_r,
                            volume: v,
                        }
                    })
                    .step(0.01)
                    .width(Length::Fill),
                ]
                .spacing(2)
                .align_y(iced::Alignment::Center);

                column![
                    row![mute_btn].align_y(iced::Alignment::Center),
                    slider_l,
                    slider_r,
                    text(if eff_pct != vol_pct {
                        format!("{}% (→{}%)", vol_pct, eff_pct)
                    } else {
                        format!("{}%", vol_pct)
                    })
                    .size(9)
                    .color(text_muted(theme_mode)),
                ]
                .spacing(1)
                .align_x(iced::Alignment::Center)
                .into()
            } else {
                // Single slider mode
                let fader = iced::widget::slider(0.0..=1.0_f32, vol, move |v| {
                    Message::RouteVolumeChanged {
                        source: src,
                        mix: mid,
                        volume: v,
                    }
                })
                .step(0.01)
                .width(Length::Fill);

                column![
                    row![mute_btn, fader]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    text(if eff_pct != vol_pct {
                        format!("{}% (→{}%)", vol_pct, eff_pct)
                    } else {
                        format!("{}%", vol_pct)
                    })
                    .size(9)
                    .color(text_muted(theme_mode)),
                ]
                .spacing(1)
                .align_x(iced::Alignment::Center)
                .into()
            }
        }
        None => {
            // Empty cell — subtle "+" that brightens on hover
            button(icon_plus().size(14).color(text_muted(theme_mode)).center())
                .width(Length::Fill)
                .height(Length::Fill)
                .on_press(Message::RouteToggled {
                    source,
                    mix: mix_id,
                })
                .padding(0)
                .style(move |_: &Theme, status| button::Style {
                    background: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(Background::Color(bg_hover(theme_mode)))
                        }
                        _ => Some(Background::Color(crate::ui::theme::bg_empty_cell(
                            theme_mode,
                        ))),
                    },
                    text_color: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            text_primary(theme_mode)
                        }
                        _ => text_muted(theme_mode),
                    },
                    border: Border {
                        color: border_color(theme_mode),
                        width: 1.0,
                        radius: CELL_RADIUS.into(),
                    },
                    ..Default::default()
                })
                .into()
        }
    };

    // Muted cells get a subtle red tint (GAP-004)
    let is_muted = route.map(|r| r.muted).unwrap_or(false);
    let cell_bg = if is_muted {
        Color {
            r: bg_elevated(theme_mode).r + 0.06,
            g: bg_elevated(theme_mode).g,
            b: bg_elevated(theme_mode).b,
            a: 1.0,
        }
    } else {
        bg_elevated(theme_mode)
    };

    let cell_h = if stereo_sliders { CELL_HEIGHT_STEREO } else { CELL_HEIGHT };
    container(cell_content)
        .width(Length::Fixed(COL_WIDTH))
        .height(Length::Fixed(cell_h))
        .padding(6)
        .center_x(Length::Fixed(COL_WIDTH))
        .center_y(Length::Fixed(cell_h))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(cell_bg)),
            border: Border {
                color: if focused {
                    ACCENT
                } else {
                    border_color(theme_mode)
                },
                width: if focused { 2.0 } else { 1.0 },
                radius: CELL_RADIUS.into(),
            },
            ..Default::default()
        })
        .into()
}
