//! Main grid layout assembly and empty-state placeholder.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Element, Length, Theme};

use crate::plugin::api::{ChannelId, MixId, MixInfo};

use crate::app::Message;
use crate::engine::state::MixerState;
use crate::plugin::api::SourceId;
use crate::ui::theme::{
    ThemeMode, bg_elevated, bg_hover, bg_primary, border_color, text_muted, text_secondary,
};

use super::{
    CELL_SPACING, COL_WIDTH,
    HEADER_HEIGHT, LABEL_WIDTH, MIX_COLORS,
};
use super::cell::matrix_cell;
use super::channel_label::channel_label;
use super::mix_header::mix_header;

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
    _seen_apps: &[String],
    monitored_mix: Option<MixId>,
    channel_master_volumes: &std::collections::HashMap<crate::plugin::api::ChannelId, f32>,
    channel_master_stereo: &std::collections::HashMap<crate::plugin::api::ChannelId, (f32, f32)>,
    stereo_sliders: bool,
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
            tracing::debug!(
                compact_mix = sel_id,
                "compact view: filtering to selected mix"
            );
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

        grid = grid.push(container(mix_picker).padding([4, 8]).width(Length::Fill));
    }

    // Header row: empty corner cell + one header per visible mix
    let mut header_row = row![
        // Corner cell (channel name column)
        container(text("").size(12))
            .width(Length::Fixed(LABEL_WIDTH))
            .height(Length::Fixed(HEADER_HEIGHT))
    ]
    .spacing(CELL_SPACING);

    let output_names: Vec<String> = state
        .hardware_outputs
        .iter()
        .map(|o| o.name.clone())
        .collect();

    for (i, mix) in visible_mixes.iter().enumerate() {
        let color = MIX_COLORS[i % MIX_COLORS.len()];
        let mix_editing = editing_mix == Some(mix.id);
        let selected_output = mix
            .output
            .and_then(|oid| state.hardware_outputs.iter().find(|o| o.id == oid))
            .map(|o| o.name.clone());
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
            i > 0, // first mix (Main/Monitor) is not removable/renameable
            &output_names,
            selected_output.as_deref(),
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

        // Master volume from UI-side HashMap (survives snapshot rebuilds)
        let first_mix_id = state.mixes.first().map(|m| m.id);
        let master_volume = channel_master_volumes
            .get(&channel.id)
            .copied()
            .unwrap_or(1.0);

        let master_stereo = channel_master_stereo.get(&channel.id).copied();
        // Solo app channels: auto-created with exactly 1 assigned binary
        // that matches the channel name. These should not be editable.
        let is_solo = channel.assigned_app_binaries.len() == 1
            && state.applications.iter().any(|a| {
                a.name.eq_ignore_ascii_case(&channel.name)
                    || a.binary.eq_ignore_ascii_case(&channel.name)
            });
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
                master_stereo,
                stereo_sliders,
                is_solo,
            ),
        ]
        .spacing(CELL_SPACING);

        for (col_index, mix) in visible_mixes.iter().enumerate() {
            let route = state.routes.get(&(source, mix.id));
            let cell_ratio = state
                .route_ratios
                .get(&(source, mix.id))
                .copied()
                .unwrap_or(1.0);
            let cell_focused = row_focused && focused_col == Some(col_index);
            ch_row = ch_row.push(matrix_cell(
                source,
                mix.id,
                route,
                cell_ratio,
                master_volume,
                peak,
                cell_focused,
                theme_mode,
                stereo_sliders,
            ));
        }

        let row_bg = if row_focused {
            bg_hover(theme_mode)
        } else {
            bg_primary(theme_mode)
        };
        grid = grid.push(container(ch_row).style(move |_: &Theme| container::Style {
            background: Some(Background::Color(row_bg)),
            ..Default::default()
        }));
    }

    // NOTE: Solo app rows are no longer rendered here as placeholders.
    // Unassigned playing apps get auto-created as real channels in PluginAppsChanged,
    // so they appear as normal channel rows with full volume control.

    // Hardware input devices shown as channel rows (mic, line-in, etc.)
    for input in &state.hardware_inputs {
        let source = SourceId::Hardware(input.id);
        let peak = state.peak_levels.get(&source).copied().unwrap_or(0.0);
        let first_mix_id = state.mixes.first().map(|m| m.id);
        let master_volume = 1.0; // hardware inputs use direct PA source volume

        let mut hw_row = row![channel_label(
            &input.name,
            false, // hardware inputs not mutable
            source,
            None, // no app icon for hardware
            theme_mode,
            false, // not editing
            "",
            &[], // no assigned apps
            0,
            peak,
            first_mix_id,
            master_volume,
            None, // hardware inputs: no stereo master
            stereo_sliders,
            false, // hardware inputs are not solo channels
        )]
        .spacing(CELL_SPACING);

        for mix in visible_mixes.iter() {
            let route = state.routes.get(&(source, mix.id));
            let hw_ratio = state
                .route_ratios
                .get(&(source, mix.id))
                .copied()
                .unwrap_or(1.0);
            hw_row = hw_row.push(matrix_cell(
                source,
                mix.id,
                route,
                hw_ratio,
                master_volume,
                peak,
                false,
                theme_mode,
                stereo_sliders,
            ));
        }

        grid = grid.push(container(hw_row).style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg_elevated(theme_mode))),
            ..Default::default()
        }));
    }

    // Channel creation: preset channels + custom name input.
    let existing_channel_names: Vec<String> = state
        .channels
        .iter()
        .map(|c| c.name.to_lowercase())
        .collect();
    grid = grid.push(super::channel_picker::channel_picker(
        show_channel_picker,
        channel_search,
        &existing_channel_names,
        theme_mode,
    ));

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
