//! Matrix grid widget — the core UI of OpenSoundGrid.
//!
//! Rows = audio sources (software channels)
//! Columns = output mixes
//! Each intersection = mute button + volume slider + VU meter

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::engine::state::MixerState;
use crate::plugin::api::SourceId;
use crate::ui::audio_slider::audio_slider;
use crate::ui::theme::{
    ACCENT, BG_ELEVATED, BG_HOVER, BG_PRIMARY, BORDER, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::ui::vu_meter::vu_meter;

/// Mix column colors, cycled for each mix.
const MIX_COLORS: &[iced::Color] = &[
    crate::ui::theme::MIX_MONITOR,
    crate::ui::theme::MIX_STREAM,
    crate::ui::theme::MIX_VOD,
    crate::ui::theme::MIX_CHAT,
    crate::ui::theme::MIX_AUX,
];

/// Build the full matrix grid from mixer state.
pub fn matrix_grid<'a>(state: &'a MixerState) -> Element<'a, Message> {
    if state.mixes.is_empty() && state.channels.is_empty() {
        return empty_matrix();
    }

    let mut grid = column![].spacing(0);

    // Header row: empty corner cell + one header per mix
    let mut header_row = row![
        // Corner cell (channel name column)
        container(text("").size(12))
            .width(Length::Fixed(120.0))
            .height(Length::Fixed(48.0))
    ]
    .spacing(1);

    for (i, mix) in state.mixes.iter().enumerate() {
        let color = MIX_COLORS[i % MIX_COLORS.len()];
        header_row = header_row.push(mix_header(&mix.name, color));
    }

    grid = grid.push(
        container(header_row)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PRIMARY)),
                ..Default::default()
            }),
    );

    // One row per channel
    for channel in &state.channels {
        let source = SourceId::Channel(channel.id);
        let peak = state.peak_levels.get(&source).copied().unwrap_or(0.0);

        let mut ch_row = row![
            // Channel name cell
            channel_label(&channel.name, channel.muted, source),
        ]
        .spacing(1);

        for mix in &state.mixes {
            let route = state.routes.get(&(source, mix.id));
            ch_row = ch_row.push(matrix_cell(source, mix.id, route, peak));
        }

        grid = grid.push(ch_row);
    }

    // "+ Create channel" button at the bottom
    let add_btn = button(
        text("+ Create channel")
            .size(12)
            .color(TEXT_SECONDARY),
    )
    .on_press(Message::CreateChannel("New".into()))
    .padding([6, 12])
    .style(|_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
            _ => None,
        },
        text_color: TEXT_SECONDARY,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });

    grid = grid.push(container(add_btn).padding([8, 0]));

    scrollable(
        container(grid)
            .padding(8)
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_ELEVATED)),
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Header cell for a mix column.
fn mix_header<'a>(name: &str, color: iced::Color) -> Element<'a, Message> {
    let color_bar = container(text("").size(1))
        .width(Length::Fill)
        .height(Length::Fixed(3.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        });

    container(
        column![
            color_bar,
            text(name.to_string())
                .size(12)
                .color(TEXT_PRIMARY)
                .center(),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fixed(140.0))
    .height(Length::Fixed(48.0))
    .padding([4, 8])
    .style(|_: &Theme| container::Style {
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

/// Channel name label on the left side of each row.
fn channel_label<'a>(name: &str, muted: bool, source: SourceId) -> Element<'a, Message> {
    let name_color = if muted { TEXT_MUTED } else { TEXT_PRIMARY };

    let mute_label = if muted { "M" } else { " " };
    let mute_btn = button(
        text(mute_label)
            .size(10)
            .center(),
    )
    .width(20)
    .height(20)
    .on_press(Message::SourceMuteToggled(source))
    .padding(0)
    .style(move |_: &Theme, _status| button::Style {
        background: if muted {
            Some(Background::Color(ACCENT))
        } else {
            None
        },
        text_color: TEXT_PRIMARY,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 2.0.into(),
        },
        ..Default::default()
    });

    container(
        row![
            mute_btn,
            text(name.to_string())
                .size(12)
                .color(name_color),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(120.0))
    .height(Length::Fixed(72.0))
    .padding([4, 8])
    .center_y(Length::Fixed(72.0))
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(BG_PRIMARY)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// A single matrix intersection cell: mute icon + slider + VU meter.
///
/// If no route exists (source not connected to mix), shows a "+" placeholder.
fn matrix_cell<'a>(
    source: SourceId,
    mix_id: u32,
    route: Option<&'a crate::plugin::api::RouteState>,
    peak: f32,
) -> Element<'a, Message> {
    let cell_content: Element<'a, Message> = match route {
        Some(route) => {
            let vol = route.volume;
            let muted = route.muted;

            let mute_label = if muted { "M" } else { " " };
            let mute_btn = button(
                text(mute_label).size(9).center(),
            )
            .width(16)
            .height(16)
            .on_press(Message::SourceMuteToggled(source))
            .padding(0)
            .style(move |_: &Theme, _status| button::Style {
                background: if muted {
                    Some(Background::Color(ACCENT))
                } else {
                    None
                },
                text_color: TEXT_PRIMARY,
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 2.0.into(),
                },
                ..Default::default()
            });

            let fader = audio_slider(vol, move |v| Message::RouteVolumeChanged {
                source,
                mix: mix_id,
                volume: v,
            });

            let meter = vu_meter(peak, 120.0, 4.0);

            column![mute_btn, fader, meter]
                .spacing(2)
                .align_x(iced::Alignment::Center)
                .into()
        }
        None => {
            // Empty cell -- source not routed to this mix
            text("+")
                .size(16)
                .color(TEXT_MUTED)
                .center()
                .into()
        }
    };

    container(cell_content)
        .width(Length::Fixed(140.0))
        .height(Length::Fixed(72.0))
        .padding(4)
        .center_x(Length::Fixed(140.0))
        .center_y(Length::Fixed(72.0))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_ELEVATED)),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Shown when the matrix is completely empty.
fn empty_matrix<'a>() -> Element<'a, Message> {
    container(
        column![
            text("No channels or mixes configured").size(14).color(TEXT_SECONDARY),
            Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
            text("Create a channel to get started").size(12).color(TEXT_MUTED),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center),
    )
    .padding(40)
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(BG_ELEVATED)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}
