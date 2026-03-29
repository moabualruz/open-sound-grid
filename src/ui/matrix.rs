//! Matrix grid widget — the core UI of Open Sound Grid.
//!
//! Rows = audio sources (software channels)
//! Columns = output mixes
//! Each intersection = mute button + volume slider + VU meter (thin bar below slider)

use std::path::PathBuf;

use iced::widget::{Space, button, column, container, image, row, scrollable, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};
use lucide_icons::iced::{
    icon_audio_waveform, icon_expand, icon_gamepad_2, icon_globe, icon_headphones, icon_mic_vocal,
    icon_music, icon_plus, icon_radio_tower, icon_search, icon_shrink, icon_sliders_vertical,
    icon_speaker, icon_users, icon_volume_2, icon_volume_x, icon_x,
};

use iced_aw::ContextMenu;

use crate::plugin::api::{ChannelId, MixId, MixInfo};

use crate::app::Message;
use crate::engine::state::MixerState;
use crate::plugin::api::SourceId;
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_elevated, bg_hover, bg_primary, border_color, text_muted, text_primary,
    text_secondary,
};
use crate::ui::vu_slider::vu_slider;

/// Height of mix column headers in pixels.
const HEADER_HEIGHT: f32 = 64.0;
/// Height of each matrix cell and channel label row in pixels.
const CELL_HEIGHT: f32 = 56.0;
/// Width of mix columns and channel label cells in pixels.
const COL_WIDTH: f32 = 150.0;
const LABEL_WIDTH: f32 = 200.0;
/// Border radius for cells, headers, and labels (WL3-style rounded cards).
const CELL_RADIUS: f32 = 8.0;
/// Spacing between cells in the grid.
const CELL_SPACING: f32 = 4.0;

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
    ("SFX", "sfx"),
    ("Aux 1", "aux"),
    ("Aux 2", "aux"),
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
    compact_view: bool,
    compact_mix: Option<MixId>,
    channel_search: &str,
    seen_apps: &[String],
    monitored_mix: Option<MixId>,
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

    // In compact view, only show the selected mix (or all if none selected)
    let visible_mixes: Vec<&MixInfo> = if compact_view {
        if let Some(sel_id) = compact_mix {
            tracing::debug!(compact_mix = sel_id, "compact view: filtering to selected mix");
            state.mixes.iter().filter(|m| m.id == sel_id).collect()
        } else {
            tracing::debug!("compact view: no mix selected, showing all mixes");
            state.mixes.iter().collect()
        }
    } else {
        state.mixes.iter().collect()
    };

    let mut grid = column![].spacing(CELL_SPACING);

    // Compact mode: mix selector dropdown at top
    if compact_view {
        let mut mix_names: Vec<String> = vec!["All channels".into()];
        mix_names.extend(state.mixes.iter().map(|m| m.name.clone()));
        let selected = compact_mix
            .and_then(|id| state.mixes.iter().find(|m| m.id == id))
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "All channels".into());

        let mix_picker = iced::widget::pick_list(mix_names, Some(selected), |name| {
            if name == "All channels" {
                Message::SelectCompactMix(None)
            } else {
                let mix_id = state.mixes.iter().find(|m| m.name == name).map(|m| m.id);
                Message::SelectCompactMix(mix_id)
            }
        })
        .text_size(13);

        grid = grid.push(
            container(mix_picker)
                .padding([4, 8])
                .width(Length::Fill),
        );
    }

    // Header row: empty corner cell + one header per visible mix
    let mut header_row = row![
        // Corner cell (channel name column)
        container(text("").size(12))
            .width(Length::Fixed(LABEL_WIDTH))
            .height(Length::Fixed(HEADER_HEIGHT))
    ]
    .spacing(CELL_SPACING);

    for (i, mix) in visible_mixes.iter().enumerate() {
        let color = MIX_COLORS[i % MIX_COLORS.len()];
        let mix_editing = editing_mix == Some(mix.id);
        header_row = header_row.push(mix_header(
            mix.id,
            &mix.name,
            color,
            mix.output.is_some(),
            mix.muted,
            monitored_mix == Some(mix.id),
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

        // Compute not-running apps: assigned binaries that aren't in the live app list
        let running_binaries: Vec<&str> = state
            .applications
            .iter()
            .map(|a| a.binary.as_str())
            .collect();
        let not_running: Vec<&str> = channel
            .assigned_app_binaries
            .iter()
            .filter(|b| !running_binaries.contains(&b.as_str()))
            .map(|b| b.as_str())
            .collect();

        // Count apps assigned to this channel (running + not-running)
        let running_assigned = state
            .applications
            .iter()
            .filter(|a| a.channel == Some(channel.id))
            .count();
        let assigned_app_count = running_assigned + not_running.len();

        // Get master volume from the first route (first mix)
        let first_mix_id = state.mixes.first().map(|m| m.id);
        let master_volume = first_mix_id
            .and_then(|mid| state.routes.get(&(source, mid)))
            .map(|r| r.volume)
            .unwrap_or(1.0);

        let mut ch_row = row![
            // Channel name cell with app icon + inline rename + master VU+slider
            channel_label(
                &channel.name,
                channel.muted,
                source,
                channel.icon_path.as_ref(),
                theme_mode,
                is_editing,
                editing_text,
                &not_running,
                assigned_app_count,
                peak,
                first_mix_id,
                master_volume,
            ),
        ]
        .spacing(CELL_SPACING);

        for (col_index, mix) in visible_mixes.iter().enumerate() {
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
        grid = grid.push(
            container(ch_row).style(move |_: &Theme| container::Style {
                background: Some(Background::Color(row_bg)),
                ..Default::default()
            }),
        );
    }

    // Channel creation: WL3-style dropdown with detected apps + preset submenu
    if show_channel_picker {
        let mut dropdown_col = column![].spacing(4);

        // Search field (WL3: search bar at top of dropdown)
        let search_input = text_input("Search apps...", channel_search)
            .on_input(Message::ChannelSearchInput)
            .size(12)
            .padding([4, 8]);
        dropdown_col = dropdown_col.push(search_input);

        // Filter apps by search text
        let search_lower = channel_search.to_lowercase();
        let filtered_apps: Vec<_> = state
            .applications
            .iter()
            .filter(|app| {
                search_lower.is_empty()
                    || app.name.to_lowercase().contains(&search_lower)
                    || app.binary.to_lowercase().contains(&search_lower)
            })
            .collect();
        tracing::debug!(total_apps = state.applications.len(), filtered = filtered_apps.len(), search = %search_lower, "channel picker: app filter applied");

        // Detected apps section (WL3: apps appear at top of dropdown)
        if !filtered_apps.is_empty() {
            dropdown_col = dropdown_col.push(
                text("Detected Apps")
                    .size(10)
                    .color(text_muted(theme_mode)),
            );
            for app in &filtered_apps {
                let stream_idx = app.stream_index;
                let app_icon: Element<'a, Message> = if let Some(ref path) = app.icon_path {
                    image(image::Handle::from_path(path))
                        .width(Length::Fixed(20.0))
                        .height(Length::Fixed(20.0))
                        .into()
                } else {
                    icon_headphones()
                        .size(14)
                        .color(text_secondary(theme_mode))
                        .center()
                        .into()
                };

                let app_btn = button(
                    row![app_icon, text(&app.name).size(11).color(text_primary(theme_mode)),]
                        .spacing(6)
                        .align_y(iced::Alignment::Center),
                )
                .on_press(Message::CreateChannelFromApp(stream_idx))
                .width(Length::Fill)
                .padding([4, 8])
                .style(move |_: &Theme, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                        _ => None,
                    },
                    text_color: text_primary(theme_mode),
                    border: Border {
                        color: border_color(theme_mode),
                        width: 0.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                });
                dropdown_col = dropdown_col.push(app_btn);
            }
        }

        // Seen-but-not-running apps (persistent history, faded)
        let running_binaries: Vec<&str> = state
            .applications
            .iter()
            .map(|a| a.binary.as_str())
            .collect();
        let not_running_seen: Vec<&String> = seen_apps
            .iter()
            .filter(|b| !running_binaries.contains(&b.as_str()))
            .filter(|b| {
                search_lower.is_empty()
                    || b.to_lowercase().contains(&search_lower)
            })
            .collect();
        if !not_running_seen.is_empty() {
            tracing::debug!(count = not_running_seen.len(), "channel picker: rendering not-running seen apps");
            for binary in &not_running_seen {
                let label = text(binary.to_string())
                    .size(11)
                    .color(text_muted(theme_mode));
                let faded_row = row![
                    icon_headphones()
                        .size(14)
                        .color(text_muted(theme_mode))
                        .center(),
                    label,
                    Space::new().width(Length::Fill),
                    text("not running").size(9).color(text_muted(theme_mode)),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center);

                dropdown_col = dropdown_col.push(
                    container(faded_row)
                        .padding([4, 8])
                        .width(Length::Fill)
                        .style(move |_: &Theme| container::Style {
                            background: Some(Background::Color(bg_primary(theme_mode))),
                            border: Border {
                                color: border_color(theme_mode),
                                width: 0.0,
                                radius: 4.0.into(),
                            },
                            ..Default::default()
                        }),
                );
            }
        }

        // Separator
        dropdown_col = dropdown_col.push(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_color(theme_mode))),
                    ..Default::default()
                }),
        );

        // Empty channel presets section (WL3: "Add empty channel" submenu)
        tracing::debug!(presets = CHANNEL_PRESETS.len(), "channel picker: rendering empty channel presets");
        dropdown_col = dropdown_col.push(
            text("Add empty channel")
                .size(10)
                .color(text_muted(theme_mode)),
        );
        let mut preset_row = row![].spacing(4);
        for &(label, _tag) in CHANNEL_PRESETS {
            let name = label.to_string();
            let btn = button(
                text(label)
                    .size(10)
                    .color(text_primary(theme_mode))
                    .center(),
            )
            .on_press(Message::CreateChannel(name))
            .padding([3, 8])
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
            preset_row = preset_row.push(btn);
        }
        dropdown_col = dropdown_col.push(scrollable(preset_row).direction(
            iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::new(),
            ),
        ));

        grid = grid.push(
            container(dropdown_col)
                .padding([8, 8])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(bg_elevated(theme_mode))),
                    border: Border {
                        color: border_color(theme_mode),
                        width: 1.0,
                        radius: CELL_RADIUS.into(),
                    },
                    ..Default::default()
                }),
        );
    } else {
        tracing::debug!("channel picker hidden: rendering create channel button");
        let add_btn = button(
            row![
                icon_plus().size(12).color(text_secondary(theme_mode)),
                text("Create channel")
                    .size(12)
                    .color(text_secondary(theme_mode)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
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
                radius: CELL_RADIUS.into(),
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
    is_monitored: bool,
    theme_mode: ThemeMode,
    editing: bool,
    editing_text: &str,
) -> Element<'a, Message> {
    tracing::trace!(mix_id, name = %name, has_output, muted, is_monitored, editing, "rendering mix header");

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

    let monitor_icon = icon_headphones().size(9).center();
    let monitor_btn = button(monitor_icon)
        .width(16)
        .height(16)
        .on_press(Message::MonitorMix(mix_id))
        .padding(0)
        .style(move |_: &Theme, _status| button::Style {
            background: if is_monitored {
                Some(Background::Color(ACCENT))
            } else {
                None
            },
            text_color: if is_monitored {
                text_primary(theme_mode)
            } else {
                text_muted(theme_mode)
            },
            border: Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

    tracing::trace!(mix_id, is_monitored, "rendering mix monitor button");

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

    // WL3 mix icon: map name prefix to icon
    let name_lower = name.to_lowercase();
    let mix_icon_el: Element<'a, Message> = if name_lower.contains("personal")
        || name_lower.contains("monitor")
    {
        icon_headphones()
            .size(16)
            .color(text_secondary(theme_mode))
            .center()
            .into()
    } else if name_lower.contains("chat") || name_lower.contains("voice") {
        icon_radio_tower()
            .size(16)
            .color(text_secondary(theme_mode))
            .center()
            .into()
    } else if name_lower.contains("stream") {
        icon_users()
            .size(16)
            .color(text_secondary(theme_mode))
            .center()
            .into()
    } else {
        icon_headphones()
            .size(16)
            .color(text_muted(theme_mode))
            .center()
            .into()
    };

    container(
        column![
            color_bar,
            row![mute_btn, monitor_btn, Space::new().width(Length::Fill), remove_btn,]
                .spacing(2)
                .align_y(iced::Alignment::Center),
            row![
                mix_icon_el,
                {
                    let mix_name_el: Element<'a, Message> = if editing {
                        text_input("Name...", editing_text)
                            .on_input(Message::RenameInput)
                            .on_submit(Message::ConfirmRename)
                            .size(11)
                            .width(Length::Fixed(80.0))
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
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
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
            radius: CELL_RADIUS.into(),
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
    not_running_apps: &[&str],
    assigned_app_count: usize,
    peak: f32,
    first_mix_id: Option<MixId>,
    master_volume: f32,
) -> Element<'a, Message> {
    tracing::trace!(name, muted, source = ?source, has_icon = icon_path.is_some(), editing, not_running = not_running_apps.len(), assigned_apps = assigned_app_count, "rendering channel label");
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

    // App icon: show resolved icon from desktop entry, or fallback headphones icon (32px WL3 parity)
    let icon_element: Element<'a, Message> = if let Some(path) = icon_path {
        image(image::Handle::from_path(path))
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(32.0))
            .into()
    } else {
        icon_headphones()
            .size(24)
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

    // Master volume slider in channel label (WL3: always visible, compact)
    let master_slider_el: Element<'a, Message> = if let Some(mix_id) = first_mix_id {
        let src = source;
        iced::widget::slider(0.0..=1.0_f32, master_volume, move |v| {
            Message::RouteVolumeChanged {
                source: src,
                mix: mix_id,
                volume: v,
            }
        })
        .step(0.01)
        .width(Length::Fixed(60.0))
        .into()
    } else {
        Space::new().width(Length::Fixed(60.0)).into()
    };

    let mut label_row = row![mute_btn, icon_element, name_element, master_slider_el,]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    // Assigned app count badge
    if assigned_app_count > 0 {
        let badge = container(
            text(format!("{}", assigned_app_count))
                .size(9)
                .color(text_primary(theme_mode))
                .center(),
        )
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .center_x(Length::Fixed(16.0))
        .center_y(Length::Fixed(16.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ACCENT)),
            border: Border {
                radius: 8.0.into(),
                ..Border::default()
            },
            ..Default::default()
        });
        label_row = label_row.push(badge);
    }

    // Not-running app indicator (GAP-017): faded text with red dot
    if !not_running_apps.is_empty() {
        let inactive_label = not_running_apps.join(", ");
        let red_dot = container(Space::new())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(crate::ui::theme::VU_RED)),
                border: Border {
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..Default::default()
            });
        label_row = label_row
            .push(red_dot)
            .push(text(inactive_label).size(9).color(text_muted(theme_mode)));
    }

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
                radius: CELL_RADIUS.into(),
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

        // Right-click context menu with Move Up/Down, Rename, Delete
        let ctx_btn_style = move |_: &Theme, status: button::Status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_primary(theme_mode),
            ..Default::default()
        };
        ContextMenu::new(clickable, move || {
            column![
                button(text("Move Up").size(11))
                    .on_press(Message::MoveChannelUp(cid))
                    .width(Length::Fill)
                    .padding([4, 12])
                    .style(ctx_btn_style),
                button(text("Move Down").size(11))
                    .on_press(Message::MoveChannelDown(cid))
                    .width(Length::Fill)
                    .padding([4, 12])
                    .style(ctx_btn_style),
                button(text("Rename").size(11))
                    .on_press(Message::StartRenameChannel(cid))
                    .width(Length::Fill)
                    .padding([4, 12])
                    .style(ctx_btn_style),
                button(text("Delete").size(11))
                    .on_press(Message::RemoveChannel(cid))
                    .width(Length::Fill)
                    .padding([4, 12])
                    .style(ctx_btn_style),
            ]
            .width(Length::Fixed(120.0))
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

            // Volume percentage label (WL3 parity)
            let vol_pct = text(format!("{}%", (vol * 100.0) as u32))
                .size(9)
                .color(text_muted(theme_mode));

            column![
                row![mute_btn, fader]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                vol_pct,
            ]
            .spacing(1)
            .align_x(iced::Alignment::Center)
            .into()
        }
        None => {
            // Empty cell — subtle "+" that brightens on hover
            button(
                icon_plus()
                    .size(14)
                    .color(text_muted(theme_mode))
                    .center(),
            )
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
                    _ => Some(Background::Color(crate::ui::theme::bg_empty_cell(theme_mode))),
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

    container(cell_content)
        .width(Length::Fixed(COL_WIDTH))
        .height(Length::Fixed(CELL_HEIGHT))
        .padding(6)
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
                radius: CELL_RADIUS.into(),
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
