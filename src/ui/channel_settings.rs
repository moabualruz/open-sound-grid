//! Channel settings side panel.
//!
//! Shows when the user clicks a channel name. Two tabs:
//! - **Apps**: detected audio apps with checkboxes to assign/unassign from this channel
//! - **Effects**: EQ, compressor, noise gate (delegates to effects_panel)

use iced::widget::{button, checkbox, column, container, image, row, text, Space};
use iced::{Background, Border, Element, Length, Theme};
use lucide_icons::iced::{icon_headphones, icon_x};

use crate::app::{ChannelPanelTab, Message};
use crate::plugin::api::{AudioApplication, ChannelInfo};
use crate::ui::theme::{
    ThemeMode, bg_elevated, bg_hover, bg_primary, border_color, text_muted, text_primary,
    text_secondary, ACCENT,
};

/// Render the channel settings side panel.
pub fn channel_settings_panel<'a>(
    channel: &'a ChannelInfo,
    apps: &'a [AudioApplication],
    not_running_binaries: Vec<String>,
    active_tab: ChannelPanelTab,
    theme_mode: ThemeMode,
    channel_name_text: &str,
) -> Element<'a, Message> {
    let ch_id = channel.id;
    // Header: channel name + close button
    let close_btn = button(icon_x().size(13).color(text_muted(theme_mode)).center())
        .width(20)
        .height(20)
        .on_press(Message::SelectedChannel(None))
        .padding(0)
        .style(move |_: &Theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(bg_hover(theme_mode))),
                _ => None,
            },
            text_color: text_muted(theme_mode),
            ..Default::default()
        });

    let header = row![
        text(&channel.name)
            .size(14)
            .color(text_primary(theme_mode)),
        Space::new().width(Length::Fill),
        close_btn,
    ]
    .align_y(iced::Alignment::Center);

    // Channel name editable field (WL3 parity: "Name:" input in settings)
    let name_field = row![
        text("Name:").size(11).color(text_muted(theme_mode)),
        iced::widget::text_input("Channel name...", channel_name_text)
            .on_input(Message::ChannelSettingsNameInput)
            .on_submit(Message::ChannelSettingsNameConfirm(ch_id))
            .size(12)
            .width(Length::Fill),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    // Tab bar
    let apps_tab = tab_button("Apps", active_tab == ChannelPanelTab::Apps, theme_mode)
        .on_press(Message::ChannelPanelTab(ChannelPanelTab::Apps));
    let effects_tab =
        tab_button("Effects", active_tab == ChannelPanelTab::Effects, theme_mode)
            .on_press(Message::ChannelPanelTab(ChannelPanelTab::Effects));

    let tab_bar = row![apps_tab, effects_tab].spacing(2);

    // Tab content
    let content: Element<'a, Message> = match active_tab {
        ChannelPanelTab::Apps => apps_tab_content(channel, apps, &not_running_binaries, theme_mode),
        ChannelPanelTab::Effects => {
            crate::ui::effects_panel::effects_panel_body(channel, theme_mode)
        }
    };

    container(
        column![header, name_field, tab_bar, content]
            .spacing(8)
            .width(Length::Fill),
    )
    .padding(12)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg_elevated(theme_mode))),
        border: Border {
            color: border_color(theme_mode),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// A tab button for the tab bar.
fn tab_button(label: &str, active: bool, theme_mode: ThemeMode) -> button::Button<'_, Message> {
    let label_color = if active {
        text_primary(theme_mode)
    } else {
        text_secondary(theme_mode)
    };

    button(text(label).size(12).color(label_color).center())
        .padding([4, 12])
        .style(move |_: &Theme, status| {
            let bg = if active {
                bg_hover(theme_mode)
            } else {
                match status {
                    button::Status::Hovered => bg_hover(theme_mode),
                    _ => bg_elevated(theme_mode),
                }
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: label_color,
                border: Border {
                    color: if active {
                        ACCENT
                    } else {
                        border_color(theme_mode)
                    },
                    width: if active { 0.0 } else { 0.0 },
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
}

/// Content for the Apps tab: list of detected apps with assignment checkboxes.
fn apps_tab_content<'a>(
    channel: &'a ChannelInfo,
    apps: &'a [AudioApplication],
    not_running_binaries: &[String],
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    let ch_id = channel.id;
    let mut col = column![
        text("Detected Applications")
            .size(11)
            .color(text_muted(theme_mode)),
    ]
    .spacing(4);

    if apps.is_empty() && not_running_binaries.is_empty() {
        col = col.push(
            text("No audio apps detected. Play audio in an app to see it here.")
                .size(11)
                .color(text_muted(theme_mode)),
        );
    }

    // Running apps
    for app in apps {
        let is_assigned = app.channel == Some(ch_id);
        let stream_idx = app.stream_index;

        let icon_el: Element<'a, Message> = if let Some(ref path) = app.icon_path {
            image::Image::new(image::Handle::from_path(path))
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0))
                .into()
        } else {
            icon_headphones()
                .size(12)
                .color(text_secondary(theme_mode))
                .center()
                .into()
        };

        let cb = checkbox(is_assigned)
            .label(&app.name)
            .on_toggle(move |checked| {
                if checked {
                    Message::AssignApp {
                        channel: ch_id,
                        stream_index: stream_idx,
                    }
                } else {
                    Message::UnassignApp {
                        channel: ch_id,
                        stream_index: stream_idx,
                    }
                }
            })
            .size(14)
            .text_size(12);

        let app_row = row![icon_el, cb]
            .spacing(6)
            .align_y(iced::Alignment::Center);

        col = col.push(
            container(app_row)
                .padding([4, 4])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(bg_primary(theme_mode))),
                    border: Border {
                        color: border_color(theme_mode),
                        width: 1.0,
                        radius: 2.0.into(),
                    },
                    ..Default::default()
                }),
        );
    }

    // Not-running apps (previously assigned but not currently active)
    for binary in not_running_binaries {
        let binary_owned = binary.clone();
        let faded_row = row![
            icon_headphones()
                .size(12)
                .color(text_muted(theme_mode))
                .center(),
            text(binary_owned)
                .size(12)
                .color(text_muted(theme_mode)),
            Space::new().width(Length::Fill),
            text("not running")
                .size(9)
                .color(text_muted(theme_mode)),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);

        col = col.push(
            container(faded_row)
                .padding([4, 4])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(bg_primary(theme_mode))),
                    border: Border {
                        color: border_color(theme_mode),
                        width: 1.0,
                        radius: 2.0.into(),
                    },
                    ..Default::default()
                }),
        );
    }

    col.into()
}
