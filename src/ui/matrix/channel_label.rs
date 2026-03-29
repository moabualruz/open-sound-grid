//! Channel left-side label rendering.

use std::path::PathBuf;

use iced::widget::{Space, button, column, container, image, row, text, text_input};
use iced::{Background, Border, Element, Length, Theme};
use lucide_icons::iced::{
    icon_headphones, icon_sliders_vertical, icon_volume_2, icon_volume_x, icon_x,
};

use iced_aw::ContextMenu;

use crate::app::Message;
use crate::plugin::api::{MixId, SourceId};
use crate::ui::theme::{
    ACCENT, ThemeMode, bg_hover, bg_primary, border_color, text_muted, text_primary,
    text_secondary,
};

use super::{CELL_HEIGHT, CELL_HEIGHT_STEREO, CELL_RADIUS, LABEL_WIDTH};

pub(super) fn channel_label<'a>(
    name: &str,
    muted: bool,
    source: SourceId,
    icon_path: Option<&PathBuf>,
    theme_mode: ThemeMode,
    editing: bool,
    editing_text: &str,
    not_running_apps: &[&str],
    assigned_app_count: usize,
    _peak: f32,
    _first_mix_id: Option<MixId>,
    master_volume: f32,
    stereo_sliders: bool,
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

    // Master volume slider — controls ALL routes for this channel proportionally
    let volume_pct = (master_volume * 100.0).round() as u32;
    let master_slider_el: Element<'a, Message> = {
        let src = source;

        if stereo_sliders {
            // L/R mode: two stacked sliders (both control same value for now)
            let src_l = source;
            let src_r = source;
            let slider_l = row![
                text("L").size(8).color(text_muted(theme_mode)),
                iced::widget::slider(0.0..=1.0_f32, master_volume, move |v| {
                    Message::ChannelMasterVolumeChanged {
                        source: src_l,
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
                iced::widget::slider(0.0..=1.0_f32, master_volume, move |v| {
                    Message::ChannelMasterVolumeChanged {
                        source: src_r,
                        volume: v,
                    }
                })
                .step(0.01)
                .width(Length::Fill),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center);

            column![
                slider_l,
                slider_r,
                text(format!("{}%", volume_pct))
                    .size(9)
                    .color(text_secondary(theme_mode)),
            ]
            .spacing(1)
            .into()
        } else {
            // Single slider mode
            let slider_widget =
                iced::widget::slider(0.0..=1.0_f32, master_volume, move |v| {
                    Message::ChannelMasterVolumeChanged {
                        source: src,
                        volume: v,
                    }
                })
                .step(0.01)
                .width(Length::Fill);

            row![
                slider_widget,
                text(format!("{}%", volume_pct))
                    .size(10)
                    .color(text_secondary(theme_mode))
                    .width(Length::Fixed(30.0)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .into()
        }
    };

    // Top row: mute + icon + name
    let mut label_row = row![mute_btn, icon_element, name_element,]
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

    // Stack: name row on top, full-width slider below
    let label_col = column![label_row, master_slider_el].spacing(2);

    let label_h = if stereo_sliders { CELL_HEIGHT_STEREO } else { CELL_HEIGHT };
    let inner = container(label_col)
        .width(Length::Fixed(LABEL_WIDTH))
        .height(Length::Fixed(label_h))
        .padding([4, 8])
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
