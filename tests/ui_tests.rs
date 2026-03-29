//! E2E UI tests for Open Sound Grid using iced_test.
//!
//! These tests run headlessly — no display server needed.
//! They create a Simulator from the app's view, interact with widgets,
//! and capture snapshots for visual regression testing.
//!
//! Run: `cargo test --test ui_tests -- --ignored`
//! Update snapshots: delete tests/snapshots/*.txt and re-run

use iced::keyboard;
use iced::{Settings, Theme};
use iced_test::{Error, Simulator};

use open_sound_grid::app::{App, ChannelPanelTab, Message};
use open_sound_grid::engine::state::MixerState;
use open_sound_grid::plugin::api::{
    AudioApplication, ChannelId, ChannelInfo, MixId, MixInfo, RouteState, SourceId,
};

use std::collections::HashMap;

/// Create a test App with seeded state (no PulseAudio connection needed).
fn test_app() -> App {
    let mut app = App::new();
    // Seed with test channels and mixes directly in engine state
    app.engine.state.channels = vec![
        ChannelInfo {
            id: 1,
            name: "Music".into(),
            apps: vec![],
            icon_path: None,
            assigned_app_binaries: vec![],
            muted: false,
            effects: Default::default(),
            master_volume: 1.0,
        },
        ChannelInfo {
            id: 2,
            name: "Game".into(),
            apps: vec![],
            icon_path: None,
            assigned_app_binaries: vec![],
            muted: false,
            effects: Default::default(),
            master_volume: 1.0,
        },
        ChannelInfo {
            id: 3,
            name: "Browser".into(),
            apps: vec![],
            icon_path: None,
            assigned_app_binaries: vec!["firefox".into()],
            muted: false,
            effects: Default::default(),
            master_volume: 1.0,
        },
    ];
    app.engine.state.mixes = vec![
        MixInfo {
            id: 1,
            name: "Monitor".into(),
            output: None,
            master_volume: 1.0,
            muted: false,
        },
        MixInfo {
            id: 2,
            name: "Stream".into(),
            output: None,
            master_volume: 1.0,
            muted: false,
        },
    ];
    // Create a route for Music -> Monitor
    app.engine.state.routes.insert(
        (SourceId::Channel(1), 1),
        RouteState {
            volume: 0.75,
            enabled: true,
            muted: false,
        ..Default::default()
        },
    );
    // Add a detected application
    app.engine.state.applications = vec![AudioApplication {
        id: 1,
        name: "Firefox".into(),
        binary: "firefox".into(),
        icon_name: Some("firefox".into()),
        icon_path: None,
        stream_index: 42,
        channel: None,
    }];
    app
}

fn sim(app: &App) -> Simulator<'_, Message> {
    Simulator::new(app.view())
}

// === Layout tests ===

#[test]
#[ignore]
fn matrix_renders_with_channels_and_mixes() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    // Channels should be visible
    ui.find("Music")?;
    ui.find("Game")?;
    ui.find("Browser")?;

    // Mixes should be visible
    ui.find("Monitor")?;
    ui.find("Stream")?;

    // Create channel button
    ui.find("Create channel")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/matrix_layout")?,
        "Matrix layout should match golden snapshot"
    );

    Ok(())
}

#[test]
#[ignore]
fn empty_matrix_shows_placeholder() -> Result<(), Error> {
    let mut app = App::new();
    // No channels, no mixes
    app.engine.state.channels.clear();
    app.engine.state.mixes.clear();

    let mut ui = sim(&app);
    ui.find("No channels or mixes configured")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/empty_matrix")?,
        "Empty matrix should match golden snapshot"
    );

    Ok(())
}

// === Mix header tests ===

#[test]
#[ignore]
fn mix_headers_show_icons() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    // Mix headers should exist with their names
    ui.find("Monitor")?;
    ui.find("Stream")?;

    // Output count badges
    ui.find("0 Outputs")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/mix_headers")?,
        "Mix headers should match golden snapshot"
    );

    Ok(())
}

// === Channel label tests ===

#[test]
#[ignore]
fn channel_labels_show_name_and_controls() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    // All channel names visible
    ui.find("Music")?;
    ui.find("Game")?;
    ui.find("Browser")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/channel_labels")?,
        "Channel labels should match golden snapshot"
    );

    Ok(())
}

// === Volume percentage tests ===

#[test]
#[ignore]
fn active_route_shows_volume_percentage() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    // Music -> Monitor route has volume 0.75 = 75%
    ui.find("75%")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/volume_percentage")?,
        "Volume percentage should match golden snapshot"
    );

    Ok(())
}

// === Channel creation dropdown tests ===

#[test]
#[ignore]
fn create_channel_dropdown_shows_apps() -> Result<(), Error> {
    let mut app = test_app();
    // Open the channel picker
    app.show_channel_picker = true;

    let mut ui = sim(&app);

    // Should show detected apps section
    ui.find("Detected Apps")?;
    ui.find("Firefox")?;

    // Should show preset section
    ui.find("Add empty channel")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/channel_dropdown")?,
        "Channel dropdown should match golden snapshot"
    );

    Ok(())
}

// === Channel settings panel tests ===

#[test]
#[ignore]
fn channel_settings_shows_name_field_and_apps() -> Result<(), Error> {
    let mut app = test_app();
    // Select channel 3 (Browser) to open settings panel
    app.selected_channel = Some(3);
    app.channel_panel_tab = ChannelPanelTab::Apps;
    app.channel_settings_name = "Browser".into();

    let mut ui = sim(&app);

    // Name field
    ui.find("Name:")?;

    // Tab buttons
    ui.find("Apps")?;
    ui.find("Effects")?;

    // Detected apps with checkboxes
    ui.find("Firefox")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/channel_settings")?,
        "Channel settings panel should match golden snapshot"
    );

    Ok(())
}

// === Shrink/expand view tests ===

#[test]
#[ignore]
fn header_shows_shrink_expand_toggle() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    // Header should show the app title
    ui.find("Open Sound Grid")?;

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/header_with_toggle")?,
        "Header should match golden snapshot"
    );

    Ok(())
}

// === Visual regression: full app ===

#[test]
#[ignore]
fn full_app_dark_theme_snapshot() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    let snapshot = ui.snapshot(&Theme::Dark)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/full_app_dark")?,
        "Full app dark theme should match golden snapshot"
    );

    Ok(())
}

#[test]
#[ignore]
fn full_app_light_theme_snapshot() -> Result<(), Error> {
    let mut app = test_app();
    app.config.ui.theme_mode = open_sound_grid::ui::theme::ThemeMode::Light;

    let mut ui = sim(&app);

    let snapshot = ui.snapshot(&Theme::Light)?;
    assert!(
        snapshot.matches_hash("tests/snapshots/full_app_light")?,
        "Full app light theme should match golden snapshot"
    );

    Ok(())
}

// === Interaction tests (no snapshots, just message verification) ===

#[test]
#[ignore]
fn clicking_create_channel_produces_toggle_message() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    let _ = ui.click("Create channel")?;

    let messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::ToggleChannelPicker)),
        "Clicking '+ Create channel' should produce ToggleChannelPicker message"
    );

    Ok(())
}

#[test]
#[ignore]
fn clicking_channel_name_produces_selected_channel() -> Result<(), Error> {
    let app = test_app();
    let mut ui = sim(&app);

    let _ = ui.click("Music")?;

    let messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::SelectedChannel(Some(1)))),
        "Clicking 'Music' should produce SelectedChannel(Some(1))"
    );

    Ok(())
}
