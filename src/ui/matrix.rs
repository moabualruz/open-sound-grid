//! Matrix grid widget — the core UI of Open Sound Grid.
//!
//! Rows = audio sources (software channels)
//! Columns = output mixes
//! Each intersection = mute button + volume slider + VU meter (thin bar below slider)

use std::path::PathBuf;

use iced::widget::{Space, button, column, container, image, row, scrollable, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};
use lucide_icons::iced::{
    icon_headphones, icon_plus, icon_sliders_vertical, icon_volume_2, icon_volume_x, icon_x,
};

use iced_aw::ContextMenu;

use crate::plugin::api::{ChannelId, MixId};

use crate::app::Message;
use crate::engine::state::MixerState;
use crate::plugin::api::SourceId;
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_elevated, bg_hover, bg_primary, border_color, text_muted, text_primary,
    text_secondary,
};
use crate::ui::vu_slider::vu_slider;

/// Height of mix column headers in pixels.
const HEADER_HEIGHT: f32 = 60.0;
/// Height of each matrix cell and channel label row in pixels.
const CELL_HEIGHT: f32 = 50.0;
/// Width of mix columns and channel label cells in pixels.
const COL_WIDTH: f32 = 140.0;
const LABEL_WIDTH: f32 = 120.0;

/// Mix column colors, cycled for each mix.
const MIX_COLORS: &[iced::Color] = &[
    crate::ui::theme::MIX_MONITOR,
    crate::ui::theme::MIX_STREAM,
    crate::ui::theme::MIX_VOD,
    crate::ui::theme::MIX_CHAT,
    crate::ui::theme::MIX_AUX,
];

/// Build the full matrix grid from mixer state.
/// Preset channel types for the creation picker.
const CHANNEL_PRESETS: &[(&str, &str)] = &[
    ("System", "system"),
    ("Game", "game"),
    ("Chat", "chat"),
    ("Music", "music"),
    ("Browser", "browser"),
    ("Voice", "voice"),
];

pub fn matrix_grid<'a>(
    state: &'a MixerState,
    focused_row: Option<usize>,
    focused_col: Option<usize>,
    theme_mode: ThemeMode,
    show_channel_picker: bool,
    editing_channel: Option<ChannelId>,
    editing_mix: Option<MixId>,
    editing_text: &str,
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
        return empty_matrix(theme_mode);
    }

    let mut grid = column![].spacing(1);

    // Header row: empty corner cell + one header per mix
    let mut header_row = row![
        // Corner cell (channel name column)
        container(text("").size(12))
            .width(Length::Fixed(LABEL_WIDTH))
            .height(Length::Fixed(HEADER_HEIGHT))
    ]
    .spacing(1);

    for (i, mix) in state.mixes.iter().enumerate() {
        let color = MIX_COLORS[i % MIX_COLORS.len()];
        let mix_editing = editing_mix == Some(mix.id);
        header_row = header_row.push(mix_header(
            mix.id,
            &mix.name,
            color,
            mix.output.is_some(),
            mix.muted,
            theme_mode,
            mix_editing,
            editing_text,
        ));
    }

    // "+ Add Mix" button in header row
    let mix_count = state.mixes.len();
    let add_mix_btn = button(
        text("+ Add Mix")
            .size(11)
            .color(text_secondary(theme_mode))
            .center(),
    )
    .on_press(Message::CreateMix(format!("Mix {}", mix_count + 1)))
    .padding([4, 8])
    .style(move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
            _ => None,
        },
        text_color: text_secondary(theme_mode),
        border: Border {
            color: border_color(theme_mode),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });

    header_row = header_row.push(
        container(add_mix_btn)
            .width(Length::Fixed(COL_WIDTH))
            .height(Length::Fixed(HEADER_HEIGHT))
            .center_x(Length::Fixed(COL_WIDTH))
            .center_y(Length::Fixed(HEADER_HEIGHT)),
    );

    grid = grid.push(
        container(header_row).style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg_primary(theme_mode))),
            ..Default::default()
        }),
    );

    // One row per channel
    for (row_index, channel) in state.channels.iter().enumerate() {
        let source = SourceId::Channel(channel.id);
        let peak = state.peak_levels.get(&source).copied().unwrap_or(0.0);
        let row_focused = focused_row == Some(row_index);

        let is_editing = editing_channel == Some(channel.id);
        let mut ch_row = row![
            // Channel name cell with app icon + inline rename
            channel_label(
                &channel.name,
                channel.muted,
                source,
                channel.icon_path.as_ref(),
                theme_mode,
                is_editing,
                editing_text,
            ),
        ]
        .spacing(1);

        for (col_index, mix) in state.mixes.iter().enumerate() {
            let route = state.routes.get(&(source, mix.id));
            let cell_focused = row_focused && focused_col == Some(col_index);
            ch_row = ch_row.push(matrix_cell(
                source,
                mix.id,
                route,
                peak,
                cell_focused,
                theme_mode,
            ));
        }

        let row_bg = if row_focused {
            bg_hover(theme_mode)
        } else {
            bg_primary(theme_mode)
        };
        grid =
            grid.push(
                container(ch_row)
                    .padding([4, 0])
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(row_bg)),
                        ..Default::default()
                    }),
            );
    }

    // Channel creation: picker toggle + preset buttons
    if show_channel_picker {
        let mut picker_row = row![].spacing(4);
        for &(label, _tag) in CHANNEL_PRESETS {
            let name = label.to_string();
            let btn = button(
                text(label)
                    .size(11)
                    .color(text_primary(theme_mode))
                    .center(),
            )
            .on_press(Message::CreateChannel(name))
            .padding([4, 10])
            .style(move |_: &Theme, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(ACCENT)),
                    _ => Some(Background::Color(bg_hover(theme_mode))),
                },
                text_color: text_primary(theme_mode),
                border: Border {
                    color: border_color(theme_mode),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });
            picker_row = picker_row.push(btn);
        }
        grid = grid.push(
            container(
                column![
                    text("Select channel type:")
                        .size(11)
                        .color(text_secondary(theme_mode)),
                    picker_row,
                ]
                .spacing(4),
            )
            .padding([8, 0]),
        );
    } else {
        let add_btn = button(
            text("+ Create channel")
                .size(12)
                .color(text_secondary(theme_mode)),
        )
        .on_press(Message::ToggleChannelPicker)
        .padding([6, 12])
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_secondary(theme_mode),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });
        grid = grid.push(container(add_btn).padding([8, 0]));
    }

    scrollable(
        container(grid)
            .padding([12, 16])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(bg_primary(theme_mode))),
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Header cell for a mix column.
///
/// Displays: colored top bar, mix name, output count badge, mute toggle, remove button.
/// No slider or VU meter — those belong in the matrix cells.
fn mix_header<'a>(
    mix_id: u32,
    name: &str,
    color: iced::Color,
    has_output: bool,
    muted: bool,
    theme_mode: ThemeMode,
    editing: bool,
    editing_text: &str,
) -> Element<'a, Message> {
    tracing::trace!(mix_id, name = %name, has_output, muted, editing, "rendering mix header");

    let color_bar = container(text("").size(1))
        .width(Length::Fill)
        .height(Length::Fixed(3.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        });

    let mute_icon = if muted {
        icon_volume_x().size(9).center()
    } else {
        icon_volume_2().size(9).center()
    };
    let mute_btn = button(mute_icon)
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
            text_color: text_primary(theme_mode),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

    let remove_btn = button(icon_x().size(10).color(text_muted(theme_mode)).center())
        .width(12)
        .height(12)
        .on_press(Message::RemoveMix(mix_id))
        .padding(0)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(Background::Color(bg_hover(theme_mode)))
                }
                _ => None,
            },
            text_color: text_muted(theme_mode),
            ..Default::default()
        });

    tracing::trace!(mix_id, "rendering mix remove button");

    let output_label = if has_output { "1 Output" } else { "0 Outputs" };

    container(
        column![
            color_bar,
            row![mute_btn, Space::new().width(Length::Fill), remove_btn,]
                .align_y(iced::Alignment::Center),
            {
                let mix_name_el: Element<'a, Message> = if editing {
                    text_input("Name...", editing_text)
                        .on_input(Message::RenameInput)
                        .on_submit(Message::ConfirmRename)
                        .size(11)
                        .width(Length::Fixed(100.0))
                        .into()
                } else {
                    button(
                        text(name.to_string())
                            .size(12)
                            .color(text_primary(theme_mode))
                            .center(),
                    )
                    .on_press(Message::StartRenameMix(mix_id))
                    .padding(0)
                    .style(|_: &Theme, _| button::Style::default())
                    .into()
                };
                mix_name_el
            },
            text(output_label)
                .size(10)
                .color(text_muted(theme_mode))
                .center(),
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fixed(COL_WIDTH))
    .height(Length::Fixed(HEADER_HEIGHT))
    .padding([4, 8])
    .style(move |_: &Theme| container::Style {
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

/// Channel name label on the left side of each row, with optional app icon.
/// When `editing` is true, shows a text_input for inline rename.
fn channel_label<'a>(
    name: &str,
    muted: bool,
    source: SourceId,
    icon_path: Option<&PathBuf>,
    theme_mode: ThemeMode,
    editing: bool,
    editing_text: &str,
) -> Element<'a, Message> {
    tracing::trace!(name, muted, source = ?source, has_icon = icon_path.is_some(), editing, "rendering channel label");
    let name_color = if muted {
        text_muted(theme_mode)
    } else {
        text_primary(theme_mode)
    };

    let channel_id = match source {
        SourceId::Channel(id) => Some(id),
        _ => None,
    };

    let mute_icon = if muted {
        icon_volume_x().size(10).center()
    } else {
        icon_volume_2().size(10).center()
    };
    let mute_btn = button(mute_icon)
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
            text_color: text_primary(theme_mode),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

    // Remove button — only shown for named channels (SourceId::Channel)
    let remove_btn: Option<Element<'a, Message>> = if let Some(cid) = channel_id {
        tracing::trace!(channel_id = cid, "rendering channel remove button");
        Some(
            button(icon_x().size(10).color(text_muted(theme_mode)).center())
                .width(12)
                .height(12)
                .on_press(Message::RemoveChannel(cid))
                .padding(0)
                .style(move |_: &Theme, status| button::Style {
                    background: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(Background::Color(bg_hover(theme_mode)))
                        }
                        _ => None,
                    },
                    text_color: text_muted(theme_mode),
                    ..Default::default()
                })
                .into(),
        )
    } else {
        None
    };

    // App icon: show resolved icon from desktop entry, or fallback headphones icon
    let icon_element: Element<'a, Message> = if let Some(path) = icon_path {
        image(image::Handle::from_path(path))
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0))
            .into()
    } else {
        icon_headphones()
            .size(14)
            .color(text_secondary(theme_mode))
            .center()
            .into()
    };

    // Name element: text_input when editing, static text otherwise
    let name_element: Element<'a, Message> = if editing {
        text_input("Name...", editing_text)
            .on_input(Message::RenameInput)
            .on_submit(Message::ConfirmRename)
            .size(12)
            .width(Length::Fixed(70.0))
            .into()
    } else {
        text(name.to_string()).size(12).color(name_color).into()
    };

    let mut label_row = row![mute_btn, icon_element, name_element,]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    // FX button — opens effects side panel for this channel (GAP-006)
    if let Some(cid) = channel_id {
        let fx_btn = button(
            icon_sliders_vertical()
                .size(10)
                .color(text_muted(theme_mode))
                .center(),
        )
        .width(16)
        .height(16)
        .on_press(Message::SelectedChannel(Some(cid)))
        .padding(0)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_muted(theme_mode),
            ..Default::default()
        });
        label_row = label_row
            .push(Space::new().width(Length::Fill))
            .push(fx_btn);
    }

    if let Some(btn) = remove_btn {
        if channel_id.is_none() {
            label_row = label_row.push(Space::new().width(Length::Fill));
        }
        label_row = label_row.push(btn);
    }

    let inner = container(label_row)
        .width(Length::Fixed(LABEL_WIDTH))
        .height(Length::Fixed(CELL_HEIGHT))
        .padding([4, 8])
        .center_y(Length::Fixed(CELL_HEIGHT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg_primary(theme_mode))),
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    if let Some(cid) = channel_id {
        tracing::trace!(
            channel_id = cid,
            "channel label is clickable, will emit SelectedChannel"
        );
        let clickable = button(inner)
            .on_press(Message::SelectedChannel(Some(cid)))
            .padding(0)
            .style(|_: &Theme, _status| button::Style {
                background: None,
                ..Default::default()
            });

        // Right-click context menu with Rename / Delete
        ContextMenu::new(clickable, move || {
            column![
                button(text("Rename").size(11))
                    .on_press(Message::StartRenameChannel(cid))
                    .width(Length::Fill)
                    .padding([4, 12])
                    .style(move |_: &Theme, status| button::Style {
                        background: match status {
                            button::Status::Hovered =>
                                Some(Background::Color(bg_hover(theme_mode))),
                            _ => None,
                        },
                        text_color: text_primary(theme_mode),
                        ..Default::default()
                    }),
                button(text("Delete").size(11))
                    .on_press(Message::RemoveChannel(cid))
                    .width(Length::Fill)
                    .padding([4, 12])
                    .style(move |_: &Theme, status| button::Style {
                        background: match status {
                            button::Status::Hovered =>
                                Some(Background::Color(bg_hover(theme_mode))),
                            _ => None,
                        },
                        text_color: text_primary(theme_mode),
                        ..Default::default()
                    }),
            ]
            .width(Length::Fixed(100.0))
            .into()
        })
        .into()
    } else {
        inner.into()
    }
}

/// A single matrix intersection cell.
///
/// Layout when routed: mute button + merged VU+Slider (VU fill IS the track background).
/// If no route exists, shows a "+" placeholder.
/// `focused` adds a 2px ACCENT border to highlight keyboard selection.
fn matrix_cell<'a>(
    source: SourceId,
    mix_id: u32,
    route: Option<&'a crate::plugin::api::RouteState>,
    peak: f32,
    focused: bool,
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    tracing::trace!(source = ?source, mix_id, has_route = route.is_some(), peak, focused, "rendering matrix cell");
    let cell_content: Element<'a, Message> = match route {
        Some(route) => {
            let vol = route.volume;
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

            // Merged VU+Slider: VU fill IS the slider track background
            let fader = vu_slider(vol, peak, muted, source, mix_id, theme_mode);

            row![mute_btn, fader]
                .spacing(4)
                .align_y(iced::Alignment::Center)
                .into()
        }
        None => {
            // Empty cell — visually recessive (darker bg, subtle icon)
            // Wave Link 3.0: empty cells are dark/inactive with no slider
            button(icon_plus().size(12).color(text_muted(theme_mode)).center())
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
                        _ => Some(Background::Color(bg_primary(theme_mode))),
                    },
                    text_color: text_muted(theme_mode),
                    border: Border {
                        color: border_color(theme_mode),
                        width: 0.0,
                        radius: 0.0.into(),
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

    container(cell_content)
        .width(Length::Fixed(COL_WIDTH))
        .height(Length::Fixed(CELL_HEIGHT))
        .padding(4)
        .center_x(Length::Fixed(COL_WIDTH))
        .center_y(Length::Fixed(CELL_HEIGHT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(cell_bg)),
            border: Border {
                color: if focused {
                    ACCENT
                } else {
                    border_color(theme_mode)
                },
                width: if focused { 2.0 } else { 1.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Shown when the matrix is completely empty.
fn empty_matrix<'a>(theme_mode: ThemeMode) -> Element<'a, Message> {
    tracing::trace!("rendering empty matrix placeholder");
    container(
        column![
            text("No channels or mixes configured")
                .size(14)
                .color(text_secondary(theme_mode)),
            Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
            text("Create a channel to get started")
                .size(12)
                .color(text_muted(theme_mode)),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center),
    )
    .padding(40)
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg_primary(theme_mode))),
        ..Default::default()
    })
    .into()
}
