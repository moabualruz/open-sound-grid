//! UI rendering: main application view.

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input, pick_list};
use iced::{Border, Element, Length, Theme};

use crate::ui;

use super::messages::{ChannelPanelTab, Message};
use super::state::App;

impl App {
    pub fn theme(&self) -> Theme {
        let resolved = ui::theme::resolve_theme(self.config.ui.theme_mode);
        match resolved {
            ui::theme::ThemeMode::Dark | ui::theme::ThemeMode::System => Theme::Dark,
            ui::theme::ThemeMode::Light => Theme::Light,
        }
    }


    pub fn view(&self) -> Element<'_, Message> {
        tracing::trace!("rendering view");

        let tm = self.config.ui.theme_mode;

        // Header — extracted to view_header.rs
        let header = self.view_header();

        // Thin separator line (1px, BORDER color)
        let sep = move || {
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_: &Theme| container::Style {
                    background: Some(iced::Background::Color(ui::theme::border_color(tm))),
                    ..Default::default()
                })
        };

        let sidebar = ui::sidebar::sidebar(
            self.sidebar_collapsed,
            &self.engine.state.hardware_inputs,
            tm,
        );

        let matrix = ui::matrix::matrix_grid(
            &self.engine.state,
            self.focused_row,
            self.focused_col,
            tm,
            self.show_channel_picker,
            self.editing_channel,
            self.editing_mix,
            &self.editing_text,
            self.compact_mix_view,
            self.compact_selected_mix,
            &self.channel_search_text,
            &self.config.seen_apps,
            self.monitored_mix,
            &self.channel_master_volumes,
            &self.channel_master_stereo,
            self.config.ui.stereo_sliders,
        );

        // App panel removed — apps auto-create channels or are managed inline
        // let app_panel = ui::app_list::app_list_panel(...);

        let connected = self.engine.is_connected();
        let channel_count = self.engine.state.channels.len();
        let route_count = self.engine.state.routes.len();
        tracing::trace!(
            connected,
            channels = channel_count,
            routes = route_count,
            "rendering status bar"
        );

        let (status_dot_color, status_text) = if connected {
            (ui::theme::STATUS_CONNECTED, "Connected")
        } else {
            (ui::theme::STATUS_ERROR, "Disconnected")
        };
        let status_dot = container(Space::new())
            .width(Length::Fixed(8.0))
            .height(Length::Fixed(8.0))
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(status_dot_color)),
                border: iced::Border {
                    radius: iced::border::Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        // Undo button (shown when undo_buffer has content)
        let undo_element: Option<Element<'_, Message>> =
            self.undo_buffer.as_ref().map(|(name, is_ch)| {
                let label = if *is_ch {
                    format!("Undo delete channel '{name}'")
                } else {
                    format!("Undo delete mix '{name}'")
                };
                button(text(label).size(10).color(ui::theme::text_primary(tm)))
                    .on_press(Message::UndoDelete)
                    .padding([2, 8])
                    .style(move |_: &Theme, status| button::Style {
                        background: match status {
                            button::Status::Hovered => {
                                Some(iced::Background::Color(ui::theme::ACCENT))
                            }
                            _ => Some(iced::Background::Color(ui::theme::bg_hover(tm))),
                        },
                        text_color: ui::theme::text_primary(tm),
                        border: Border {
                            color: ui::theme::border_color(tm),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            });

        // Status bar — same bg as header/sidebar for visual frame
        let mut status_row = row![
            status_dot,
            Space::new().width(Length::Fixed(6.0)),
            text(status_text)
                .size(11)
                .color(ui::theme::text_secondary(tm)),
        ]
        .align_y(iced::Alignment::Center);

        if let Some(undo) = undo_element {
            status_row = status_row
                .push(Space::new().width(Length::Fixed(12.0)))
                .push(undo);
        }

        // Focused cell coordinates (Journey 12: keyboard power user)
        if let (Some(r), Some(c)) = (self.focused_row, self.focused_col) {
            let ch_name = self
                .engine
                .state
                .channels
                .get(r)
                .map(|ch| ch.name.as_str())
                .unwrap_or("?");
            let mix_name = self
                .engine
                .state
                .mixes
                .get(c)
                .map(|m| m.name.as_str())
                .unwrap_or("?");
            status_row = status_row
                .push(Space::new().width(Length::Fixed(12.0)))
                .push(
                    text(format!("{} × {}", ch_name, mix_name))
                        .size(11)
                        .color(ui::theme::ACCENT),
                );
        }

        // Right side of status bar kept minimal — detailed stats moved to settings
        status_row = status_row.push(Space::new().width(Length::Fill));

        let status_bar = container(status_row.padding([4, 16]))
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::bg_secondary(tm))),
                ..Default::default()
            });

        // Settings overlay — shown when settings_open is true
        let settings_panel: Option<Element<'_, Message>> = if self.settings_open {
            tracing::trace!(settings_open = true, "rendering settings panel");
            let latency_val = self.config.audio.latency_ms.to_string();

            // Preset save row: text input + "Save" button
            let preset_name_val = self.preset_name_input.clone();
            let save_btn = button(text("Save").size(12))
                .on_press(Message::SavePreset(self.preset_name_input.clone()))
                .padding([2, 8]);
            let preset_save_row = row![
                text_input("Preset name…", &preset_name_val)
                    .on_input(Message::PresetNameInput)
                    .size(12)
                    .padding([2, 6]),
                Space::new().width(Length::Fixed(6.0)),
                save_btn,
            ]
            .align_y(iced::Alignment::Center);

            // Preset load row: pick_list + "Load" button
            let selected_preset: Option<String> = None;
            let preset_names = self.available_presets.clone();
            let load_btn = button(text("Load").size(12))
                .on_press_maybe(
                    selected_preset
                        .as_ref()
                        .map(|n| Message::LoadPreset(n.clone())),
                )
                .padding([2, 8]);
            let preset_load_row = row![
                pick_list(preset_names, selected_preset, Message::LoadPreset)
                    .placeholder("Select preset…")
                    .text_size(12),
                Space::new().width(Length::Fixed(6.0)),
                load_btn,
            ]
            .align_y(iced::Alignment::Center);

            let panel = container(
                column![
                    text("Settings").size(14).color(ui::theme::text_primary(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
                    row![
                        text("Latency (ms): ")
                            .size(12)
                            .color(ui::theme::text_secondary(tm)),
                        text_input("20", &latency_val)
                            .on_input(Message::LatencyInput)
                            .size(12)
                            .padding([2, 6])
                            .width(Length::Fixed(50.0)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                    {
                        // PipeWire latency note
                        let pw_socket = std::env::var("XDG_RUNTIME_DIR")
                            .map(|d| std::path::PathBuf::from(d).join("pipewire-0"))
                            .ok();
                        let pw_active = pw_socket.as_ref().map_or(false, |p| p.exists());
                        if pw_active {
                            text("PipeWire active — latency is managed by PipeWire graph scheduler (typically <5ms)")
                                .size(10)
                                .color(ui::theme::ACCENT)
                        } else {
                            text("Tip: Install PipeWire for significantly reduced audio latency")
                                .size(10)
                                .color(ui::theme::text_muted(tm))
                        }
                    },
                    text(format!("Config: ~/.config/open-sound-grid/"))
                        .size(11)
                        .color(ui::theme::text_muted(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    button(
                        text(if self.config.ui.stereo_sliders {
                            "Sliders: L/R (Stereo)"
                        } else {
                            "Sliders: Single (Mono)"
                        })
                        .size(12),
                    )
                    .on_press(Message::ToggleStereoSliders)
                    .padding([4, 8]),
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    text(format!(
                        "Plugin: {}",
                        if self.engine.is_connected() {
                            "PulseAudio"
                        } else {
                            "None"
                        }
                    ))
                    .size(12)
                    .color(ui::theme::text_secondary(tm)),
                    text(format!(
                        "Channels: {} / Mixes: {}",
                        self.engine.state.channels.len(),
                        self.engine.state.mixes.len()
                    ))
                    .size(12)
                    .color(ui::theme::text_secondary(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(8.0)),
                    text("Presets").size(13).color(ui::theme::text_primary(tm)),
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    preset_save_row,
                    Space::new().width(Length::Fill).height(Length::Fixed(4.0)),
                    preset_load_row,
                ]
                .spacing(4),
            )
            .padding(12)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::bg_elevated(tm))),
                border: iced::Border {
                    color: ui::theme::border_color(tm),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });
            Some(panel.into())
        } else {
            None
        };

        // Build the matrix area: matrix on the left, optional channel settings panel on the right
        let matrix_area: Element<'_, Message> = if let Some(ch_id) = self.selected_channel {
            if let Some(ch) = self.engine.state.channels.iter().find(|c| c.id == ch_id) {
                // Compute not-running binaries for this channel
                let running_binaries: Vec<&str> = self
                    .engine
                    .state
                    .applications
                    .iter()
                    .map(|a| a.binary.as_str())
                    .collect();
                let not_running: Vec<String> = ch
                    .assigned_app_binaries
                    .iter()
                    .filter(|b| !running_binaries.contains(&b.as_str()))
                    .cloned()
                    .collect();

                tracing::trace!(channel_id = ch_id, "rendering channel settings side panel");
                let side_panel = scrollable(ui::channel_settings::channel_settings_panel(
                    ch,
                    &self.engine.state.applications,
                    not_running,
                    self.channel_panel_tab,
                    tm,
                    &self.channel_settings_name,
                ))
                .width(Length::Fixed(280.0))
                .height(Length::Fill);

                row![matrix, side_panel,]
                    .spacing(0)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                matrix
            }
        } else {
            matrix
        };

        // Right panel: flush stack — header, sep, matrix+effects, [settings], app panel, sep, status
        let mut right_panel = column![header, sep(), matrix_area]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        // Settings panel before app panel
        if let Some(settings) = settings_panel {
            right_panel = right_panel.push(settings);
        }

        // App panel removed — apps auto-create channels

        // Status bar removed — info moved to settings panel

        let content = row![sidebar, right_panel];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(ui::theme::bg_primary(tm))),
                border: iced::Border {
                    radius: iced::border::Radius {
                        top_left: 0.0,
                        top_right: 0.0,
                        bottom_right: 8.0,
                        bottom_left: 8.0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}
