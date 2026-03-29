//! Per-channel effects chain panel.
//!
//! Shows EQ, compressor, and noise gate controls for the selected channel.
//! Parameters are stored and sent to the plugin; audio processing is wired
//! when PA stream capture is available.

use iced::widget::{Space, button, column, container, row, slider, text, toggler};
use iced::{Background, Element, Length, Theme};
use lucide_icons::iced::icon_x;

use crate::app::Message;
use crate::effects::EffectsParams;
use crate::plugin::api::ChannelInfo;
use crate::ui::eq_widget::eq_canvas;
use crate::ui::theme::{
    ThemeMode, bg_elevated, bg_hover, border_color, text_muted, text_primary, text_secondary,
};

/// Render the standalone effects panel for a selected channel (legacy, with own header/close).
pub fn effects_panel<'a>(channel: &'a ChannelInfo, theme_mode: ThemeMode) -> Element<'a, Message> {
    let ch_id = channel.id;
    let params = &channel.effects;
    tracing::trace!(channel_id = ch_id, name = %channel.name, effects_enabled = params.enabled, "rendering effects panel");

    let close_btn = button(icon_x().size(13).color(text_muted(theme_mode)).center())
        .width(20)
        .height(20)
        .on_press(Message::SelectedChannel(None))
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

    tracing::trace!(channel_id = ch_id, "rendering effects panel close button");

    let header = row![
        text("Effects").size(13).color(text_primary(theme_mode)),
        Space::new().width(Length::Fill),
        toggler(params.enabled)
            .on_toggle(move |enabled| Message::EffectsToggled {
                channel: ch_id,
                enabled
            })
            .size(16),
        close_btn,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8);

    let channel_label = text(format!("Channel: {}", channel.name))
        .size(12)
        .color(text_primary(theme_mode));

    let body = effects_controls(channel, theme_mode);

    let panel = column![
        channel_label,
        Space::new().height(Length::Fixed(4.0)),
        header,
        body,
    ]
    .spacing(2)
    .padding(8);

    container(panel)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(iced::Background::Color(bg_elevated(theme_mode))),
            border: iced::Border {
                color: border_color(theme_mode),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Effects panel body without header/close — used by channel_settings panel.
pub fn effects_panel_body<'a>(
    channel: &'a ChannelInfo,
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    let ch_id = channel.id;
    let params = &channel.effects;
    tracing::trace!(channel_id = ch_id, name = %channel.name, effects_enabled = params.enabled, "rendering effects_panel_body");

    let toggle_row = row![
        text("Effects").size(12).color(text_primary(theme_mode)),
        Space::new().width(Length::Fill),
        toggler(params.enabled)
            .on_toggle(move |enabled| Message::EffectsToggled {
                channel: ch_id,
                enabled,
            })
            .size(16),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8);

    let body = effects_controls(channel, theme_mode);

    column![toggle_row, body].spacing(4).into()
}

/// The actual EQ/compressor/gate controls — shared by both panel variants.
fn effects_controls<'a>(
    channel: &'a ChannelInfo,
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    let ch_id = channel.id;
    let params = &channel.effects;
    tracing::trace!(channel_id = ch_id, "rendering effects_controls");

    let sep = move || {
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(border_color(theme_mode))),
                ..Default::default()
            })
    };

    // --- EQ section ---
    let eq_label = text("Parametric EQ")
        .size(11)
        .color(text_secondary(theme_mode));

    let eq_freq_label = text(format!("Freq: {:.0} Hz", params.eq_freq_hz))
        .size(11)
        .color(text_muted(theme_mode));
    let eq_freq = {
        let id = ch_id;
        slider(20.0_f32..=20000.0, params.eq_freq_hz, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "eq_freq_hz".into(),
                value: v,
            }
        })
        .step(1.0)
    };

    let eq_q_label = text(format!("Q: {:.2}", params.eq_q))
        .size(11)
        .color(text_muted(theme_mode));
    let eq_q = {
        let id = ch_id;
        slider(0.1_f32..=10.0, params.eq_q, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "eq_q".into(),
                value: v,
            }
        })
        .step(0.01)
    };

    let eq_gain_label = text(format!("Gain: {:.1} dB", params.eq_gain_db))
        .size(11)
        .color(text_muted(theme_mode));
    let eq_gain = {
        let id = ch_id;
        slider(-24.0_f32..=24.0, params.eq_gain_db, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "eq_gain_db".into(),
                value: v,
            }
        })
        .step(0.1)
    };

    // Placeholder — will be replaced with real FFT data from PA stream capture in v0.4.
    let spectrum: Vec<(f32, f32)> = vec![];
    let eq_viz = eq_canvas(ch_id, params, &spectrum);
    tracing::trace!(channel_id = ch_id, "rendering eq_viz canvas");

    // --- Compressor section ---
    let comp_label = text("Compressor")
        .size(11)
        .color(text_secondary(theme_mode));

    let comp_thresh_label = text(format!("Threshold: {:.1} dB", params.comp_threshold_db))
        .size(11)
        .color(text_muted(theme_mode));
    let comp_thresh = {
        let id = ch_id;
        slider(-60.0_f32..=0.0, params.comp_threshold_db, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "comp_threshold_db".into(),
                value: v,
            }
        })
        .step(0.5)
    };

    let comp_ratio_label = text(format!("Ratio: {:.1}:1", params.comp_ratio))
        .size(11)
        .color(text_muted(theme_mode));
    let comp_ratio = {
        let id = ch_id;
        slider(1.0_f32..=20.0, params.comp_ratio, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "comp_ratio".into(),
                value: v,
            }
        })
        .step(0.1)
    };

    let comp_attack_label = text(format!("Attack: {:.1} ms", params.comp_attack_ms))
        .size(11)
        .color(text_muted(theme_mode));
    let comp_attack = {
        let id = ch_id;
        slider(0.1_f32..=100.0, params.comp_attack_ms, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "comp_attack_ms".into(),
                value: v,
            }
        })
        .step(0.1)
    };

    let comp_release_label = text(format!("Release: {:.0} ms", params.comp_release_ms))
        .size(11)
        .color(text_muted(theme_mode));
    let comp_release = {
        let id = ch_id;
        slider(10.0_f32..=1000.0, params.comp_release_ms, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "comp_release_ms".into(),
                value: v,
            }
        })
        .step(1.0)
    };

    // --- Noise gate section ---
    let gate_label = text("Noise Gate")
        .size(11)
        .color(text_secondary(theme_mode));

    let gate_thresh_label = text(format!("Threshold: {:.1} dB", params.gate_threshold_db))
        .size(11)
        .color(text_muted(theme_mode));
    let gate_thresh = {
        let id = ch_id;
        slider(-80.0_f32..=0.0, params.gate_threshold_db, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "gate_threshold_db".into(),
                value: v,
            }
        })
        .step(0.5)
    };

    let gate_hold_label = text(format!("Hold: {:.0} ms", params.gate_hold_ms))
        .size(11)
        .color(text_muted(theme_mode));
    let gate_hold = {
        let id = ch_id;
        slider(1.0_f32..=500.0, params.gate_hold_ms, move |v| {
            Message::EffectsParamChanged {
                channel: id,
                param: "gate_hold_ms".into(),
                value: v,
            }
        })
        .step(1.0)
    };

    column![
        sep(),
        Space::new().height(Length::Fixed(4.0)),
        eq_label,
        eq_freq_label,
        eq_freq,
        eq_q_label,
        eq_q,
        eq_gain_label,
        eq_gain,
        Space::new().height(Length::Fixed(6.0)),
        eq_viz,
        Space::new().height(Length::Fixed(4.0)),
        sep(),
        Space::new().height(Length::Fixed(4.0)),
        comp_label,
        comp_thresh_label,
        comp_thresh,
        comp_ratio_label,
        comp_ratio,
        comp_attack_label,
        comp_attack,
        comp_release_label,
        comp_release,
        Space::new().height(Length::Fixed(4.0)),
        sep(),
        Space::new().height(Length::Fixed(4.0)),
        gate_label,
        gate_thresh_label,
        gate_thresh,
        gate_hold_label,
        gate_hold,
    ]
    .spacing(2)
    .into()
}

/// Build a modified `EffectsParams` with the named param replaced by `value`.
/// Returns `None` if `param` is not a recognized field name.
pub fn apply_param_change(
    params: &EffectsParams,
    param: &str,
    value: f32,
) -> Option<EffectsParams> {
    let mut p = params.clone();
    match param {
        "eq_freq_hz" => p.eq_freq_hz = value,
        "eq_q" => p.eq_q = value,
        "eq_gain_db" => p.eq_gain_db = value,
        "comp_threshold_db" => p.comp_threshold_db = value,
        "comp_ratio" => p.comp_ratio = value,
        "comp_attack_ms" => p.comp_attack_ms = value,
        "comp_release_ms" => p.comp_release_ms = value,
        "gate_threshold_db" => p.gate_threshold_db = value,
        "gate_hold_ms" => p.gate_hold_ms = value,
        _ => return None,
    }
    Some(p)
}
