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
pub fn matrix_grid<'a>(
    state: &'a MixerState,
    focused_row: Option<usize>,
    focused_col: Option<usize>,
) -> Element<'a, Message> {
    tracing::trace!(
        channels = state.channels.len(),
        mixes = state.mixes.len(),
        routes = state.routes.len(),
        focused_row = ?focused_row,
        focused_col = ?focused_col,
        "rendering matrix grid"
    );
    if state.mixes.is_empty() && state.channels.is_empty() {
        return empty_matrix();
    }

    let mut grid = column![].spacing(1);

    // Header row: empty corner cell + one header per mix
    let mut header_row = row![
        // Corner cell (channel name column)
        container(text("").size(12))
            .width(Length::Fixed(120.0))
            .height(Length::Fixed(96.0))
    ]
    .spacing(1);

    for (i, mix) in state.mixes.iter().enumerate() {
        let color = MIX_COLORS[i % MIX_COLORS.len()];
        let mix_peak = state.peak_levels.get(&SourceId::Mix(mix.id)).copied().unwrap_or(0.0);
        header_row = header_row.push(mix_header(
            mix.id,
            &mix.name,
            color,
            mix.master_volume,
            mix.muted,
            mix_peak,
        ));
    }

    // "+ Add Mix" button in header row
    let mix_count = state.mixes.len();
    let add_mix_btn = button(
        text("+ Add Mix")
            .size(11)
            .color(TEXT_SECONDARY)
            .center(),
    )
    .on_press(Message::CreateMix(format!("Mix {}", mix_count + 1)))
    .padding([4, 8])
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

    header_row = header_row.push(
        container(add_mix_btn)
            .width(Length::Fixed(140.0))
            .height(Length::Fixed(96.0))
            .center_x(Length::Fixed(140.0))
            .center_y(Length::Fixed(96.0)),
    );

    grid = grid.push(
        container(header_row)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PRIMARY)),
                ..Default::default()
            }),
    );

    // One row per channel
    for (row_index, channel) in state.channels.iter().enumerate() {
        let source = SourceId::Channel(channel.id);
        let peak = state.peak_levels.get(&source).copied().unwrap_or(0.0);
        let row_focused = focused_row == Some(row_index);

        let mut ch_row = row![
            // Channel name cell
            channel_label(&channel.name, channel.muted, source),
        ]
        .spacing(1);

        for (col_index, mix) in state.mixes.iter().enumerate() {
            let route = state.routes.get(&(source, mix.id));
            let cell_focused = row_focused && focused_col == Some(col_index);
            ch_row = ch_row.push(matrix_cell(source, mix.id, route, peak, cell_focused));
        }

        let row_bg = if row_focused { BG_HOVER } else { BG_PRIMARY };
        grid = grid.push(
            container(ch_row)
                .padding([4, 0])
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(row_bg)),
                    ..Default::default()
                }),
        );
    }

    // "+ Create channel" button at the bottom
    let add_btn = button(
        text("+ Create channel")
            .size(12)
            .color(TEXT_SECONDARY),
    )
    .on_press(Message::CreateChannel(format!("Channel {}", state.channels.len() + 1)))
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
            .padding([12, 16])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PRIMARY)),
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Header cell for a mix column.
fn mix_header<'a>(
    mix_id: u32,
    name: &str,
    color: iced::Color,
    master_volume: f32,
    muted: bool,
    peak: f32,
) -> Element<'a, Message> {
    tracing::trace!(mix_id, name = %name, volume = master_volume, muted, peak, "rendering mix header");

    let color_bar = container(text("").size(1))
        .width(Length::Fill)
        .height(Length::Fixed(3.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        });

    let mute_label = if muted { "M" } else { " " };
    let mute_btn = button(
        text(mute_label)
            .size(9)
            .center(),
    )
    .width(16)
    .height(16)
    .on_press(Message::MixMuteToggled(mix_id))
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

    let volume_slider = audio_slider(master_volume, move |v| {
        Message::MixMasterVolumeChanged {
            mix: mix_id,
            volume: v,
        }
    });

    let remove_btn = button(
        text("×")
            .size(10)
            .color(TEXT_MUTED)
            .center(),
    )
    .width(12)
    .height(12)
    .on_press(Message::RemoveMix(mix_id))
    .padding(0)
    .style(|_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => {
                Some(Background::Color(BG_HOVER))
            }
            _ => None,
        },
        text_color: TEXT_MUTED,
        ..Default::default()
    });

    tracing::trace!(mix_id, "rendering mix remove button");

    let meter = vu_meter(peak, 120.0, 4.0);

    container(
        column![
            color_bar,
            row![
                Space::new().width(Length::Fill),
                remove_btn,
            ]
            .align_y(iced::Alignment::Center),
            text(name.to_string())
                .size(12)
                .color(TEXT_PRIMARY)
                .center(),
            row![mute_btn, volume_slider]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            meter,
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fixed(140.0))
    .height(Length::Fixed(96.0))
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
    tracing::trace!(name, muted, source = ?source, "rendering channel label");
    let name_color = if muted { TEXT_MUTED } else { TEXT_PRIMARY };

    let channel_id = match source {
        SourceId::Channel(id) => Some(id),
        _ => None,
    };

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

    // Remove button — only shown for named channels (SourceId::Channel)
    let remove_btn: Option<Element<'a, Message>> = if let Some(cid) = channel_id {
        tracing::trace!(channel_id = cid, "rendering channel remove button");
        Some(
            button(
                text("×")
                    .size(10)
                    .color(TEXT_MUTED)
                    .center(),
            )
            .width(12)
            .height(12)
            .on_press(Message::RemoveChannel(cid))
            .padding(0)
            .style(|_: &Theme, status| button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(BG_HOVER))
                    }
                    _ => None,
                },
                text_color: TEXT_MUTED,
                ..Default::default()
            })
            .into(),
        )
    } else {
        None
    };

    let mut label_row = row![
        mute_btn,
        text(name.to_string())
            .size(12)
            .color(name_color),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    if let Some(btn) = remove_btn {
        label_row = label_row.push(Space::new().width(Length::Fill)).push(btn);
    }

    let inner = container(label_row)
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(96.0))
        .padding([4, 8])
        .center_y(Length::Fixed(96.0))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_PRIMARY)),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    if let Some(cid) = channel_id {
        tracing::trace!(channel_id = cid, "channel label is clickable, will emit SelectedChannel");
        button(inner)
            .on_press(Message::SelectedChannel(Some(cid)))
            .padding(0)
            .style(|_: &Theme, _status| button::Style {
                background: None,
                ..Default::default()
            })
            .into()
    } else {
        inner.into()
    }
}

/// A single matrix intersection cell: mute icon + slider + VU meter.
///
/// If no route exists (source not connected to mix), shows a "+" placeholder.
/// `focused` adds a 2px ACCENT border to highlight keyboard selection.
fn matrix_cell<'a>(
    source: SourceId,
    mix_id: u32,
    route: Option<&'a crate::plugin::api::RouteState>,
    peak: f32,
    focused: bool,
) -> Element<'a, Message> {
    tracing::trace!(source = ?source, mix_id, has_route = route.is_some(), peak, focused, "rendering matrix cell");
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
            .on_press(Message::RouteMuteToggled { source, mix: mix_id })
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
            // Empty cell -- source not routed to this mix; clicking creates the route
            button(
                text("+")
                    .size(16)
                    .color(TEXT_MUTED)
                    .center(),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::RouteToggled { source, mix: mix_id })
            .padding(0)
            .style(|_: &Theme, status| button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(ACCENT))
                    }
                    _ => Some(Background::Color(BG_ELEVATED)),
                },
                text_color: TEXT_MUTED,
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into()
        }
    };

    container(cell_content)
        .width(Length::Fixed(140.0))
        .height(Length::Fixed(96.0))
        .padding(4)
        .center_x(Length::Fixed(140.0))
        .center_y(Length::Fixed(96.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(BG_ELEVATED)),
            border: Border {
                color: if focused { ACCENT } else { BORDER },
                width: if focused { 2.0 } else { 1.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Shown when the matrix is completely empty.
fn empty_matrix<'a>() -> Element<'a, Message> {
    tracing::trace!("rendering empty matrix placeholder");
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
        background: Some(Background::Color(BG_PRIMARY)),
        ..Default::default()
    })
    .into()
}
