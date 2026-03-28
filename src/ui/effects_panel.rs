//! Per-channel effects chain panel.
//!
//! Shows EQ, compressor, and noise gate controls for the selected channel.
//! Parameters are stored and sent to the plugin; audio processing is wired
//! when PA stream capture is available.

use iced::widget::{column, container, row, slider, text, toggler, Space};
use iced::{Element, Length, Theme};

use crate::app::Message;
use crate::effects::EffectsParams;
use crate::plugin::api::ChannelInfo;
use crate::ui::theme;

/// Render the effects panel for a selected channel.
pub fn effects_panel<'a>(channel: &'a ChannelInfo) -> Element<'a, Message> {
    let ch_id = channel.id;
    let params = &channel.effects;

    let header = row![
        text("Effects").size(13).color(theme::TEXT_PRIMARY),
        Space::new().width(Length::Fill),
        toggler(params.enabled)
            .on_toggle(move |enabled| Message::EffectsToggled { channel: ch_id, enabled })
            .size(16),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8);

    let sep = || {
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(|_: &Theme| container::Style {
                background: Some(iced::Background::Color(theme::BORDER)),
                ..Default::default()
            })
    };

    // --- EQ section ---
    let eq_label = text("Parametric EQ").size(11).color(theme::TEXT_SECONDARY);

    let eq_freq_label = text(format!("Freq: {:.0} Hz", params.eq_freq_hz))
        .size(11)
        .color(theme::TEXT_MUTED);
    let eq_freq = {
        let id = ch_id;
        slider(20.0_f32..=20000.0, params.eq_freq_hz, move |v| {
            Message::EffectsParamChanged { channel: id, param: "eq_freq_hz".into(), value: v }
        })
        .step(1.0)
    };

    let eq_q_label = text(format!("Q: {:.2}", params.eq_q))
        .size(11)
        .color(theme::TEXT_MUTED);
    let eq_q = {
        let id = ch_id;
        slider(0.1_f32..=10.0, params.eq_q, move |v| {
            Message::EffectsParamChanged { channel: id, param: "eq_q".into(), value: v }
        })
        .step(0.01)
    };

    let eq_gain_label = text(format!("Gain: {:.1} dB", params.eq_gain_db))
        .size(11)
        .color(theme::TEXT_MUTED);
    let eq_gain = {
        let id = ch_id;
        slider(-24.0_f32..=24.0, params.eq_gain_db, move |v| {
            Message::EffectsParamChanged { channel: id, param: "eq_gain_db".into(), value: v }
        })
        .step(0.1)
    };

    // --- Compressor section ---
    let comp_label = text("Compressor").size(11).color(theme::TEXT_SECONDARY);

    let comp_thresh_label = text(format!("Threshold: {:.1} dB", params.comp_threshold_db))
        .size(11)
        .color(theme::TEXT_MUTED);
    let comp_thresh = {
        let id = ch_id;
        slider(-60.0_f32..=0.0, params.comp_threshold_db, move |v| {
            Message::EffectsParamChanged { channel: id, param: "comp_threshold_db".into(), value: v }
        })
        .step(0.5)
    };

    let comp_ratio_label = text(format!("Ratio: {:.1}:1", params.comp_ratio))
        .size(11)
        .color(theme::TEXT_MUTED);
    let comp_ratio = {
        let id = ch_id;
        slider(1.0_f32..=20.0, params.comp_ratio, move |v| {
            Message::EffectsParamChanged { channel: id, param: "comp_ratio".into(), value: v }
        })
        .step(0.1)
    };

    // --- Noise gate section ---
    let gate_label = text("Noise Gate").size(11).color(theme::TEXT_SECONDARY);

    let gate_thresh_label = text(format!("Threshold: {:.1} dB", params.gate_threshold_db))
        .size(11)
        .color(theme::TEXT_MUTED);
    let gate_thresh = {
        let id = ch_id;
        slider(-80.0_f32..=0.0, params.gate_threshold_db, move |v| {
            Message::EffectsParamChanged { channel: id, param: "gate_threshold_db".into(), value: v }
        })
        .step(0.5)
    };

    let channel_label = text(format!("Channel: {}", channel.name))
        .size(12)
        .color(theme::TEXT_PRIMARY);

    let panel = column![
        channel_label,
        Space::new().height(Length::Fixed(4.0)),
        header,
        sep(),
        Space::new().height(Length::Fixed(4.0)),
        eq_label,
        eq_freq_label,
        eq_freq,
        eq_q_label,
        eq_q,
        eq_gain_label,
        eq_gain,
        Space::new().height(Length::Fixed(4.0)),
        sep(),
        Space::new().height(Length::Fixed(4.0)),
        comp_label,
        comp_thresh_label,
        comp_thresh,
        comp_ratio_label,
        comp_ratio,
        Space::new().height(Length::Fixed(4.0)),
        sep(),
        Space::new().height(Length::Fixed(4.0)),
        gate_label,
        gate_thresh_label,
        gate_thresh,
    ]
    .spacing(2)
    .padding(8);

    container(panel)
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(iced::Background::Color(theme::BG_ELEVATED)),
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Build a modified `EffectsParams` with the named param replaced by `value`.
/// Returns `None` if `param` is not a recognized field name.
pub fn apply_param_change(params: &EffectsParams, param: &str, value: f32) -> Option<EffectsParams> {
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
