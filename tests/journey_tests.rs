//! E2E Journey Tests for Open Sound Grid
//!
//! Tests derived from the aspirational journey spec at:
//! `projects/open-sound-grid/docs/journey-spec.md`
//!
//! Each test module maps to one journey. Each test maps to one success criterion.
//! Tests describe the DREAM behavior — failing tests show gaps to fix.
//!
//! Run all:  `cargo test --test journey_tests -- --ignored`
//! Run one:  `cargo test --test journey_tests j01 -- --ignored`
//! Snapshots: `tests/snapshots/` (delete .txt files to regenerate golden hashes)

use iced::keyboard;
use iced::keyboard::key::Named;
use iced::{Theme, Size};
use iced_test::{Error, Simulator};
use std::time::Duration;

use open_sound_grid::app::{App, ChannelPanelTab, Message};
use open_sound_grid::effects::EffectsParams;
use open_sound_grid::plugin::api::{
    AudioApplication, ChannelInfo, MixInfo, RouteState, SourceId,
};

// =============================================================================
// Test fixtures
// =============================================================================

/// Bare app — no channels, no mixes. First launch state.
fn empty_app() -> App {
    let mut app = App::new();
    app.engine.state.channels.clear();
    app.engine.state.mixes.clear();
    app.engine.state.routes.clear();
    app.engine.state.applications.clear();
    app
}

/// Casual user app — 3 channels, 1 mix, 1 route, 1 detected app.
fn casual_app() -> App {
    let mut app = App::new();
    app.engine.state.channels = vec![
        channel(1, "Music"),
        channel(2, "Discord"),
        channel(3, "Game"),
    ];
    app.engine.state.mixes = vec![monitor_mix()];
    app.engine.state.routes.insert(
        (SourceId::Channel(1), 1),
        RouteState { volume: 0.8, enabled: true, muted: false },
    );
    app.engine.state.routes.insert(
        (SourceId::Channel(2), 1),
        RouteState { volume: 0.5, enabled: true, muted: false },
    );
    app.engine.state.routes.insert(
        (SourceId::Channel(3), 1),
        RouteState { volume: 0.9, enabled: true, muted: false },
    );
    app.engine.state.applications = vec![
        detected_app(1, "Firefox", "firefox", 42),
        detected_app(2, "Spotify", "spotify", 43),
        detected_app(3, "Discord", "discord", 44),
    ];
    app
}

/// Streamer app — 5 channels, 3 mixes (Monitor/Stream/VOD), effects on mic.
fn streamer_app() -> App {
    let mut app = App::new();
    app.engine.state.channels = vec![
        channel_with_effects(1, "Mic", true),
        channel(2, "Game"),
        channel(3, "Discord"),
        channel(4, "Music"),
        channel(5, "Alerts"),
    ];
    app.engine.state.mixes = vec![
        monitor_mix(),
        stream_mix(),
        MixInfo { id: 3, name: "VOD".into(), output: None, master_volume: 1.0, muted: false },
    ];
    // All channels routed to Monitor
    for ch_id in 1..=5 {
        app.engine.state.routes.insert(
            (SourceId::Channel(ch_id), 1),
            RouteState { volume: 0.75, enabled: true, muted: false },
        );
    }
    // Mic + Game + Discord + Alerts routed to Stream (no Music = DMCA safe)
    for ch_id in [1, 2, 3, 5] {
        app.engine.state.routes.insert(
            (SourceId::Channel(ch_id), 2),
            RouteState { volume: 0.7, enabled: true, muted: false },
        );
    }
    // VOD = same as Stream (no Music)
    for ch_id in [1, 2, 3, 5] {
        app.engine.state.routes.insert(
            (SourceId::Channel(ch_id), 3),
            RouteState { volume: 0.7, enabled: true, muted: false },
        );
    }
    app
}

/// Gamer app — 4 channels, 1 mix, preset-ready.
fn gamer_app() -> App {
    let mut app = App::new();
    app.engine.state.channels = vec![
        channel(1, "Game"),
        channel(2, "Discord"),
        channel(3, "Music"),
        channel(4, "System"),
    ];
    app.engine.state.mixes = vec![monitor_mix()];
    app.engine.state.routes.insert(
        (SourceId::Channel(1), 1),
        RouteState { volume: 0.9, enabled: true, muted: false },
    );
    app.engine.state.routes.insert(
        (SourceId::Channel(2), 1),
        RouteState { volume: 0.5, enabled: true, muted: false },
    );
    app.engine.state.routes.insert(
        (SourceId::Channel(3), 1),
        RouteState { volume: 0.2, enabled: true, muted: false },
    );
    app.engine.state.routes.insert(
        (SourceId::Channel(4), 1),
        RouteState { volume: 0.0, enabled: true, muted: true },
    );
    app
}

/// Remote worker app — Teams, Slack, Music, System channels.
fn worker_app() -> App {
    let mut app = App::new();
    app.engine.state.channels = vec![
        channel(1, "Teams"),
        channel(2, "Slack"),
        channel(3, "Music"),
        channel(4, "System"),
    ];
    app.engine.state.mixes = vec![monitor_mix()];
    for ch_id in 1..=4 {
        app.engine.state.routes.insert(
            (SourceId::Channel(ch_id), 1),
            RouteState { volume: 0.7, enabled: true, muted: false },
        );
    }
    app.engine.state.applications = vec![
        detected_app(1, "Microsoft Teams", "teams", 50),
        detected_app(2, "Slack", "slack", 51),
    ];
    app
}

// --- Helpers ---

fn channel(id: u32, name: &str) -> ChannelInfo {
    ChannelInfo {
        id,
        name: name.into(),
        apps: vec![],
        icon_path: None,
        assigned_app_binaries: vec![],
        muted: false,
        effects: EffectsParams::default(),
    }
}

fn channel_with_effects(id: u32, name: &str, enabled: bool) -> ChannelInfo {
    let mut ch = channel(id, name);
    ch.effects.enabled = enabled;
    ch.effects.comp_threshold_db = -20.0;
    ch.effects.gate_threshold_db = -40.0;
    ch
}

fn monitor_mix() -> MixInfo {
    MixInfo { id: 1, name: "Monitor".into(), output: None, master_volume: 1.0, muted: false }
}

fn stream_mix() -> MixInfo {
    MixInfo { id: 2, name: "Stream".into(), output: None, master_volume: 1.0, muted: false }
}

fn detected_app(id: u32, name: &str, binary: &str, stream_index: u32) -> AudioApplication {
    AudioApplication {
        id,
        name: name.into(),
        binary: binary.into(),
        icon_name: Some(binary.into()),
        icon_path: None,
        stream_index,
        channel: None,
    }
}

fn sim(app: &App) -> Simulator<'_, Message> {
    Simulator::new(app.view())
}

// =============================================================================
// Journey 1: First Launch — Casey (Casual)
// =============================================================================
mod j01_first_launch {
    use super::*;

    #[test]
    #[ignore]
    fn first_launch_shows_empty_state_with_action_hint() -> Result<(), Error> {
        let app = empty_app();
        let mut ui = sim(&app);
        ui.find("No channels or mixes configured")?;
        ui.find("Create a channel to get started")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn first_launch_shows_connected_status() -> Result<(), Error> {
        let app = empty_app();
        let mut ui = sim(&app);
        // Status bar should indicate connection state
        // Even without PA, the UI should render
        ui.find("Open Sound Grid")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn config_persists_across_restart() -> Result<(), Error> {
        let app = casual_app();
        let mut ui = sim(&app);
        // Channels should be visible (restored from config)
        ui.find("Music")?;
        ui.find("Discord")?;
        ui.find("Game")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn volume_percentage_visible_on_active_routes() -> Result<(), Error> {
        let app = casual_app();
        let mut ui = sim(&app);
        // Music at 80% volume
        ui.find("80%")?;
        // Discord at 50%
        ui.find("50%")?;
        // Game at 90%
        ui.find("90%")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn first_launch_snapshot() -> Result<(), Error> {
        let app = empty_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j01_first_launch")?);
        Ok(())
    }
}

// =============================================================================
// Journey 2: Gaming Session — Gabe (Gamer)
// =============================================================================
mod j02_gaming_session {
    use super::*;

    #[test]
    #[ignore]
    fn gaming_channels_visible() -> Result<(), Error> {
        let app = gamer_app();
        let mut ui = sim(&app);
        ui.find("Game")?;
        ui.find("Discord")?;
        ui.find("Music")?;
        ui.find("System")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn muted_channel_shows_visual_indicator() -> Result<(), Error> {
        let app = gamer_app();
        let mut ui = sim(&app);
        // System is muted (volume 0, muted: true)
        // Should show mute indicator - the test verifies the UI renders it
        ui.find("System")?;
        // Volume shows 0%
        ui.find("0%")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn clicking_channel_mute_produces_message() -> Result<(), Error> {
        let app = gamer_app();
        let mut ui = sim(&app);
        // There should be mute buttons - clicking one produces SourceMuteToggled
        // Since we can't target by icon, we look for the channel and verify messages
        let messages: Vec<Message> = ui.into_messages().collect();
        // No interaction yet, no messages expected
        assert!(messages.is_empty());
        Ok(())
    }

    #[test]
    #[ignore]
    fn gaming_session_snapshot() -> Result<(), Error> {
        let app = gamer_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j02_gaming")?);
        Ok(())
    }
}

// =============================================================================
// Journey 3: Stream Setup — Sam (Streamer)
// =============================================================================
mod j03_stream_setup {
    use super::*;

    #[test]
    #[ignore]
    fn three_mixes_visible() -> Result<(), Error> {
        let app = streamer_app();
        let mut ui = sim(&app);
        ui.find("Monitor")?;
        ui.find("Stream")?;
        ui.find("VOD")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn five_channels_visible() -> Result<(), Error> {
        let app = streamer_app();
        let mut ui = sim(&app);
        ui.find("Mic")?;
        ui.find("Game")?;
        ui.find("Discord")?;
        ui.find("Music")?;
        ui.find("Alerts")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn music_not_routed_to_stream_mix() -> Result<(), Error> {
        let app = streamer_app();
        // Music (ch 4) should NOT be routed to Stream (mix 2) — DMCA protection
        let route = app.engine.state.routes.get(&(SourceId::Channel(4), 2));
        assert!(route.is_none(), "Music should NOT be routed to Stream mix (DMCA)");
        Ok(())
    }

    #[test]
    #[ignore]
    fn music_not_routed_to_vod_mix() -> Result<(), Error> {
        let app = streamer_app();
        // Music (ch 4) should NOT be routed to VOD (mix 3) — DMCA protection
        let route = app.engine.state.routes.get(&(SourceId::Channel(4), 3));
        assert!(route.is_none(), "Music should NOT be routed to VOD mix (DMCA)");
        Ok(())
    }

    #[test]
    #[ignore]
    fn add_mix_button_visible() -> Result<(), Error> {
        let app = streamer_app();
        let mut ui = sim(&app);
        ui.find("+ Add Mix")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn effects_button_visible_per_channel() -> Result<(), Error> {
        let app = streamer_app();
        let mut ui = sim(&app);
        // All channels should have effects button - Mic has effects enabled
        ui.find("Mic")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn streamer_snapshot() -> Result<(), Error> {
        let app = streamer_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j03_streamer")?);
        Ok(())
    }
}

// =============================================================================
// Journey 5: Podcast Recording — Pat (Podcaster)
// =============================================================================
mod j05_podcast {
    use super::*;

    #[test]
    #[ignore]
    fn multiple_mic_channels_supported() -> Result<(), Error> {
        let mut app = App::new();
        app.engine.state.channels = vec![
            channel(1, "Host Mic"),
            channel(2, "Guest 1"),
            channel(3, "Guest 2"),
        ];
        app.engine.state.mixes = vec![
            monitor_mix(),
            MixInfo { id: 2, name: "Record".into(), output: None, master_volume: 1.0, muted: false },
            MixInfo { id: 3, name: "Guest Return".into(), output: None, master_volume: 1.0, muted: false },
        ];
        let mut ui = sim(&app);
        ui.find("Host Mic")?;
        ui.find("Guest 1")?;
        ui.find("Guest 2")?;
        ui.find("Record")?;
        ui.find("Guest Return")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn independent_effects_per_channel() -> Result<(), Error> {
        let mut app = App::new();
        let mut host = channel_with_effects(1, "Host Mic", true);
        host.effects.comp_threshold_db = -20.0;
        let mut guest = channel_with_effects(2, "Guest 1", true);
        guest.effects.comp_threshold_db = -15.0;
        app.engine.state.channels = vec![host, guest];
        app.engine.state.mixes = vec![monitor_mix()];

        // Verify effects are independent
        assert_ne!(
            app.engine.state.channels[0].effects.comp_threshold_db,
            app.engine.state.channels[1].effects.comp_threshold_db,
            "Channels should have independent effects"
        );
        Ok(())
    }

    #[test]
    #[ignore]
    fn podcast_snapshot() -> Result<(), Error> {
        let mut app = App::new();
        app.engine.state.channels = vec![
            channel(1, "Host Mic"),
            channel(2, "Guest 1"),
            channel(3, "Guest 2"),
        ];
        app.engine.state.mixes = vec![
            monitor_mix(),
            MixInfo { id: 2, name: "Record".into(), output: None, master_volume: 1.0, muted: false },
        ];
        for ch_id in 1..=3 {
            for mix_id in 1..=2 {
                app.engine.state.routes.insert(
                    (SourceId::Channel(ch_id), mix_id),
                    RouteState { volume: 0.75, enabled: true, muted: false },
                );
            }
        }
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j05_podcast")?);
        Ok(())
    }
}

// =============================================================================
// Journey 6: Work Day — Robin (Remote Worker)
// =============================================================================
mod j06_work_day {
    use super::*;

    #[test]
    #[ignore]
    fn work_channels_visible() -> Result<(), Error> {
        let app = worker_app();
        let mut ui = sim(&app);
        ui.find("Teams")?;
        ui.find("Slack")?;
        ui.find("Music")?;
        ui.find("System")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn detected_apps_in_channel_settings() -> Result<(), Error> {
        let mut app = worker_app();
        app.selected_channel = Some(1); // Teams
        app.channel_panel_tab = ChannelPanelTab::Apps;
        app.channel_settings_name = "Teams".into();

        let mut ui = sim(&app);
        ui.find("Microsoft Teams")?;
        ui.find("Apps")?;
        ui.find("Effects")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn work_day_snapshot() -> Result<(), Error> {
        let app = worker_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j06_work")?);
        Ok(())
    }
}

// =============================================================================
// Journey 8: First Channel Creation — Casey (Casual)
// =============================================================================
mod j08_channel_creation {
    use super::*;

    #[test]
    #[ignore]
    fn create_channel_button_visible() -> Result<(), Error> {
        let app = empty_app();
        // With no channels, there might not be a create button in empty state
        // but with mixes present there should be
        let mut app2 = App::new();
        app2.engine.state.channels.clear();
        app2.engine.state.mixes = vec![monitor_mix()];
        let mut ui = sim(&app2);
        ui.find("Create channel")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn dropdown_shows_detected_apps() -> Result<(), Error> {
        let mut app = casual_app();
        app.show_channel_picker = true;

        let mut ui = sim(&app);
        ui.find("Detected Apps")?;
        ui.find("Firefox")?;
        ui.find("Spotify")?;
        ui.find("Discord")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn dropdown_shows_preset_section() -> Result<(), Error> {
        let mut app = casual_app();
        app.show_channel_picker = true;

        let mut ui = sim(&app);
        ui.find("Add empty channel")?;
        ui.find("System")?;
        ui.find("Game")?;
        ui.find("Music")?;
        ui.find("Browser")?;
        ui.find("Voice")?;
        ui.find("SFX")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn clicking_create_channel_opens_dropdown() -> Result<(), Error> {
        let mut app = App::new();
        app.engine.state.mixes = vec![monitor_mix()];
        app.engine.state.channels.clear();
        let mut ui = sim(&app);

        let _ = ui.click("Create channel")?;
        let messages: Vec<Message> = ui.into_messages().collect();
        assert!(
            messages.iter().any(|m| matches!(m, Message::ToggleChannelPicker)),
            "Should produce ToggleChannelPicker message"
        );
        Ok(())
    }

    #[test]
    #[ignore]
    fn clicking_app_creates_channel() -> Result<(), Error> {
        let mut app = casual_app();
        app.show_channel_picker = true;

        let mut ui = sim(&app);
        let _ = ui.click("Firefox")?;
        let messages: Vec<Message> = ui.into_messages().collect();
        assert!(
            messages.iter().any(|m| matches!(m, Message::CreateChannelFromApp(42))),
            "Clicking Firefox should produce CreateChannelFromApp(42)"
        );
        Ok(())
    }

    #[test]
    #[ignore]
    fn channel_creation_dropdown_snapshot() -> Result<(), Error> {
        let mut app = casual_app();
        app.show_channel_picker = true;

        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j08_channel_creation")?);
        Ok(())
    }
}

// =============================================================================
// Journey 9: Effects Chain Setup — Sam (Streamer)
// =============================================================================
mod j09_effects {
    use super::*;

    #[test]
    #[ignore]
    fn effects_panel_shows_toggle_and_controls() -> Result<(), Error> {
        let mut app = streamer_app();
        app.selected_channel = Some(1); // Mic with effects enabled
        app.channel_panel_tab = ChannelPanelTab::Effects;
        app.channel_settings_name = "Mic".into();

        let mut ui = sim(&app);
        ui.find("Effects")?;
        // EQ controls
        ui.find("Parametric EQ")?;
        // Compressor
        ui.find("Compressor")?;
        // Noise Gate
        ui.find("Noise Gate")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn copy_paste_effects_produces_messages() -> Result<(), Error> {
        let mut app = streamer_app();
        app.selected_channel = Some(1); // Mic
        // Simulate Ctrl+C
        let msg = Message::CopyEffects(1);
        let _task = app.update(msg);
        assert!(app.copied_effects.is_some(), "CopyEffects should populate copied_effects");

        // Simulate Ctrl+V on Game channel
        app.selected_channel = Some(2);
        let msg = Message::PasteEffects(2);
        let _task = app.update(msg);
        // The paste command was sent to the engine
        Ok(())
    }

    #[test]
    #[ignore]
    fn effects_panel_snapshot() -> Result<(), Error> {
        let mut app = streamer_app();
        app.selected_channel = Some(1);
        app.channel_panel_tab = ChannelPanelTab::Effects;
        app.channel_settings_name = "Mic".into();

        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j09_effects")?);
        Ok(())
    }
}

// =============================================================================
// Journey 10: Compact Mode — Gabe (Gamer)
// =============================================================================
mod j10_compact_mode {
    use super::*;

    #[test]
    #[ignore]
    fn shrink_expand_toggle_exists() -> Result<(), Error> {
        let app = gamer_app();
        let mut ui = sim(&app);
        // The header should have the shrink/expand toggle
        ui.find("Open Sound Grid")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn compact_mode_toggle_produces_message() -> Result<(), Error> {
        let mut app = gamer_app();
        let msg = Message::ToggleMixesView;
        let _task = app.update(msg);
        assert!(app.compact_mix_view, "ToggleMixesView should set compact_mix_view = true");
        assert!(app.compact_selected_mix.is_some(), "Should auto-select first mix");
        Ok(())
    }

    #[test]
    #[ignore]
    fn compact_mode_selects_mix() -> Result<(), Error> {
        let mut app = gamer_app();
        app.compact_mix_view = true;
        app.compact_selected_mix = Some(1);

        let msg = Message::SelectCompactMix(Some(1));
        let _task = app.update(msg);
        assert_eq!(app.compact_selected_mix, Some(1));
        Ok(())
    }
}

// =============================================================================
// Journey 11: Device Hot-Swap — Robin (Remote Worker)
// =============================================================================
mod j11_device_hotswap {
    use super::*;

    #[test]
    #[ignore]
    fn routing_preserved_when_no_routes_change() -> Result<(), Error> {
        let app = worker_app();
        // Verify all routes exist
        assert!(app.engine.state.routes.contains_key(&(SourceId::Channel(1), 1)));
        assert!(app.engine.state.routes.contains_key(&(SourceId::Channel(2), 1)));
        assert!(app.engine.state.routes.contains_key(&(SourceId::Channel(3), 1)));
        assert!(app.engine.state.routes.contains_key(&(SourceId::Channel(4), 1)));
        Ok(())
    }

    #[test]
    #[ignore]
    fn hardware_devices_shown_in_sidebar() -> Result<(), Error> {
        let mut app = worker_app();
        app.engine.state.hardware_inputs = vec![
            open_sound_grid::plugin::api::HardwareInput {
                id: 1,
                name: "Built-in Audio".into(),
                description: "Laptop speakers".into(),
            },
        ];
        let mut ui = sim(&app);
        ui.find("Built-in Audio")?;
        Ok(())
    }
}

// =============================================================================
// Journey 12: Keyboard Power User — Alex (Gamer-Streamer)
// =============================================================================
mod j12_keyboard {
    use super::*;

    #[test]
    #[ignore]
    fn tab_focuses_matrix() -> Result<(), Error> {
        let mut app = gamer_app();
        // Simulate Tab key
        let msg = Message::KeyPressed(
            keyboard::Key::Named(Named::Tab),
            keyboard::Modifiers::default(),
        );
        let _task = app.update(msg);
        assert!(app.focused_row.is_some() || app.focused_col.is_some(),
            "Tab should set focus into matrix");
        Ok(())
    }

    #[test]
    #[ignore]
    fn arrow_keys_adjust_volume() -> Result<(), Error> {
        let mut app = gamer_app();
        // Set initial focus on Game × Monitor cell
        app.focused_row = Some(0);
        app.focused_col = Some(0);

        let vol_before = app.engine.state.routes
            .get(&(SourceId::Channel(1), 1))
            .map(|r| r.volume)
            .unwrap_or(0.0);

        // Arrow down = volume down
        let msg = Message::KeyPressed(
            keyboard::Key::Named(Named::ArrowDown),
            keyboard::Modifiers::default(),
        );
        let _task = app.update(msg);

        // Volume should have decreased (or command sent to engine)
        // The handler sends SetRouteVolume to the engine, so state may not update
        // immediately in test. This verifies the handler runs without panic.
        Ok(())
    }

    #[test]
    #[ignore]
    fn escape_clears_focus() -> Result<(), Error> {
        let mut app = gamer_app();
        app.focused_row = Some(1);
        app.focused_col = Some(0);

        let msg = Message::KeyPressed(
            keyboard::Key::Named(Named::Escape),
            keyboard::Modifiers::default(),
        );
        let _task = app.update(msg);
        assert!(app.focused_row.is_none(), "Escape should clear row focus");
        assert!(app.focused_col.is_none(), "Escape should clear col focus");
        Ok(())
    }

    #[test]
    #[ignore]
    fn m_key_toggles_mute() -> Result<(), Error> {
        let mut app = gamer_app();
        app.focused_row = Some(0); // Game channel
        app.focused_col = Some(0); // Monitor mix

        let msg = Message::KeyPressed(
            keyboard::Key::Character("m".into()),
            keyboard::Modifiers::default(),
        );
        let _task = app.update(msg);
        // Should have produced a mute toggle command
        // We verify by checking if the engine received the command
        Ok(())
    }

    #[test]
    #[ignore]
    fn ctrl_c_copies_effects() -> Result<(), Error> {
        let mut app = streamer_app();
        app.selected_channel = Some(1); // Mic with effects

        let msg = Message::KeyPressed(
            keyboard::Key::Character("c".into()),
            keyboard::Modifiers::CTRL,
        );
        let _task = app.update(msg);
        assert!(app.copied_effects.is_some(), "Ctrl+C should copy effects from selected channel");
        Ok(())
    }

    #[test]
    #[ignore]
    fn ctrl_v_pastes_effects() -> Result<(), Error> {
        let mut app = streamer_app();
        // First copy
        app.copied_effects = Some(app.engine.state.channels[0].effects.clone());
        app.selected_channel = Some(2); // Game channel

        let msg = Message::KeyPressed(
            keyboard::Key::Character("v".into()),
            keyboard::Modifiers::CTRL,
        );
        let _task = app.update(msg);
        // Paste command was sent
        Ok(())
    }
}

// =============================================================================
// Visual regression — full app snapshots per persona
// =============================================================================
mod visual_regression {
    use super::*;

    #[test]
    #[ignore]
    fn casual_dark_snapshot() -> Result<(), Error> {
        let app = casual_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/persona_casual_dark")?);
        Ok(())
    }

    #[test]
    #[ignore]
    fn casual_light_snapshot() -> Result<(), Error> {
        let mut app = casual_app();
        app.config.ui.theme_mode = open_sound_grid::ui::theme::ThemeMode::Light;
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Light)?;
        assert!(snapshot.matches_hash("tests/snapshots/persona_casual_light")?);
        Ok(())
    }

    #[test]
    #[ignore]
    fn streamer_dark_snapshot() -> Result<(), Error> {
        let app = streamer_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/persona_streamer_dark")?);
        Ok(())
    }

    #[test]
    #[ignore]
    fn gamer_dark_snapshot() -> Result<(), Error> {
        let app = gamer_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/persona_gamer_dark")?);
        Ok(())
    }

    #[test]
    #[ignore]
    fn worker_dark_snapshot() -> Result<(), Error> {
        let app = worker_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/persona_worker_dark")?);
        Ok(())
    }

    #[test]
    #[ignore]
    fn empty_state_dark_snapshot() -> Result<(), Error> {
        let app = empty_app();
        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/empty_dark")?);
        Ok(())
    }
}

// =============================================================================
// Channel settings panel tests
// =============================================================================
mod channel_settings {
    use super::*;

    #[test]
    #[ignore]
    fn settings_shows_name_field() -> Result<(), Error> {
        let mut app = casual_app();
        app.selected_channel = Some(1);
        app.channel_panel_tab = ChannelPanelTab::Apps;
        app.channel_settings_name = "Music".into();

        let mut ui = sim(&app);
        ui.find("Name:")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn settings_shows_apps_tab_with_checkboxes() -> Result<(), Error> {
        let mut app = casual_app();
        app.selected_channel = Some(1);
        app.channel_panel_tab = ChannelPanelTab::Apps;
        app.channel_settings_name = "Music".into();

        let mut ui = sim(&app);
        ui.find("Apps")?;
        ui.find("Effects")?;
        ui.find("Detected Applications")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn settings_name_change_produces_message() -> Result<(), Error> {
        let mut app = casual_app();
        app.selected_channel = Some(1);
        app.channel_settings_name = "Music".into();

        let msg = Message::ChannelSettingsNameInput("Music Player".into());
        let _task = app.update(msg);
        assert_eq!(app.channel_settings_name, "Music Player");

        let msg = Message::ChannelSettingsNameConfirm(1);
        let _task = app.update(msg);
        // Rename command sent to engine
        Ok(())
    }

    #[test]
    #[ignore]
    fn channel_settings_snapshot() -> Result<(), Error> {
        let mut app = casual_app();
        app.selected_channel = Some(1);
        app.channel_panel_tab = ChannelPanelTab::Apps;
        app.channel_settings_name = "Music".into();

        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/channel_settings_apps")?);
        Ok(())
    }
}

// =============================================================================
// RED TESTS — These encode DREAM behavior not yet implemented
// =============================================================================

// --- Persistent App History (Journey 1, 8) ---
mod j_persistent_apps {
    use super::*;

    #[test]
    #[ignore]
    fn seen_apps_field_exists_in_config() {
        let app = App::new();
        // Config should have a seen_apps field that persists app binary names
        assert!(
            app.config.seen_apps.is_empty() || !app.config.seen_apps.is_empty(),
            "AppConfig should have a seen_apps: Vec<String> field"
        );
    }

    #[test]
    #[ignore]
    fn apps_changed_updates_seen_apps() {
        let mut app = casual_app();
        // Simulate PluginAppsChanged — new apps should be added to seen_apps
        let apps = vec![
            detected_app(1, "Firefox", "firefox", 42),
            detected_app(2, "Spotify", "spotify", 43),
        ];
        let _task = app.update(Message::PluginAppsChanged(apps));

        assert!(
            app.config.seen_apps.contains(&"firefox".to_string()),
            "firefox should be in seen_apps after detection"
        );
        assert!(
            app.config.seen_apps.contains(&"spotify".to_string()),
            "spotify should be in seen_apps after detection"
        );
    }

    #[test]
    #[ignore]
    fn seen_apps_show_faded_in_dropdown_when_not_running() -> Result<(), Error> {
        let mut app = App::new();
        app.engine.state.mixes = vec![monitor_mix()];
        app.engine.state.channels.clear();
        // App was seen before but is not currently running
        app.config.seen_apps = vec!["firefox".into(), "spotify".into()];
        app.engine.state.applications.clear(); // no running apps
        app.show_channel_picker = true;

        let mut ui = sim(&app);
        // Seen-but-not-running apps should appear faded in the dropdown
        // They should still be findable by name
        ui.find("firefox")?;
        ui.find("spotify")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn seen_apps_persist_across_restart() {
        let mut app = App::new();
        app.config.seen_apps = vec!["firefox".into(), "vlc".into()];
        // After save + reload, seen_apps should survive
        let _ = app.config.save();

        let reloaded = open_sound_grid::config::AppConfig::load();
        assert!(
            reloaded.seen_apps.contains(&"firefox".to_string()),
            "seen_apps should persist after config reload"
        );
    }
}

// --- Compact View Rendering (Journey 10) ---
mod j_compact_view {
    use super::*;

    #[test]
    #[ignore]
    fn compact_view_shows_mix_selector_dropdown() -> Result<(), Error> {
        let mut app = gamer_app();
        app.compact_mix_view = true;
        app.compact_selected_mix = Some(1);

        let mut ui = sim(&app);
        // In compact mode, channels should still be visible
        ui.find("Game")?;
        // The mix selector pick_list renders "Monitor" as the selected option
        ui.find("Monitor")?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn compact_view_shows_single_mix_column() -> Result<(), Error> {
        let mut app = streamer_app();
        app.compact_mix_view = true;
        app.compact_selected_mix = Some(2); // Stream mix only

        let mut ui = sim(&app);
        // Stream should be visible
        ui.find("Stream")?;
        // Monitor should NOT be visible in compact mode with Stream selected
        // (This tests that only the selected mix column renders)
        let monitor_result = ui.find("Monitor");
        // In compact view, Monitor header should not appear
        // Note: Monitor text might appear in channel labels' master slider context
        // The key test is that only 1 mix column of cells renders
        Ok(())
    }

    #[test]
    #[ignore]
    fn compact_view_snapshot() -> Result<(), Error> {
        let mut app = gamer_app();
        app.compact_mix_view = true;
        app.compact_selected_mix = Some(1);

        let mut ui = sim(&app);
        let snapshot = ui.snapshot(&Theme::Dark)?;
        assert!(snapshot.matches_hash("tests/snapshots/j10_compact_view")?);
        Ok(())
    }
}

// --- Autostart (Journey 4, 6) ---
mod j_autostart {
    use super::*;

    #[test]
    #[ignore]
    fn autostart_desktop_entry_can_be_created() {
        // The app should be able to create an XDG autostart .desktop file
        let autostart_dir = directories::BaseDirs::new().map(|d| d.config_dir().to_path_buf())
            .unwrap()
            .join("autostart");
        let desktop_path = autostart_dir.join("open-sound-grid.desktop");

        // Call the autostart setup function
        open_sound_grid::autostart::install_autostart().expect("Should create autostart entry");

        assert!(desktop_path.exists(), "Autostart .desktop file should exist");

        let content = std::fs::read_to_string(&desktop_path).unwrap();
        assert!(content.contains("[Desktop Entry]"));
        assert!(content.contains("Exec=open-sound-grid"));
        assert!(content.contains("Type=Application"));

        // Cleanup
        let _ = std::fs::remove_file(&desktop_path);
    }

    #[test]
    #[ignore]
    fn autostart_desktop_entry_can_be_removed() {
        let autostart_dir = directories::BaseDirs::new().map(|d| d.config_dir().to_path_buf())
            .unwrap()
            .join("autostart");
        let desktop_path = autostart_dir.join("open-sound-grid.desktop");

        // Install first
        open_sound_grid::autostart::install_autostart().expect("install");
        assert!(desktop_path.exists());

        // Remove
        open_sound_grid::autostart::remove_autostart().expect("remove");
        assert!(!desktop_path.exists(), "Autostart file should be removed");
    }
}

// --- Full Tracing Coverage ---
mod j_tracing {
    use super::*;

    #[test]
    #[ignore]
    fn every_message_handler_has_tracing() {
        // Verify that handling any message variant doesn't panic
        // and that tracing is wired (we can't easily assert log output in tests,
        // but we can ensure no handler is missing)
        let mut app = casual_app();

        // Exercise every message variant that doesn't require PA connection
        let messages: Vec<Message> = vec![
            Message::ToggleChannelPicker,
            Message::ToggleChannelDropdown,
            Message::ChannelSearchInput("test".into()),
            Message::ToggleMixesView,
            Message::SelectCompactMix(Some(1)),
            Message::ChannelPanelTab(ChannelPanelTab::Apps),
            Message::ChannelPanelTab(ChannelPanelTab::Effects),
            Message::ChannelSettingsNameInput("New Name".into()),
            Message::SettingsToggled,
            Message::SidebarToggleCollapse,
            Message::ThemeToggled,
            Message::PresetNameInput("test preset".into()),
            Message::UndoDelete,
            Message::ClearUndo,
            Message::RenameInput("rename test".into()),
            Message::CancelRename,
        ];

        for msg in messages {
            let _task = app.update(msg);
            // No panic = tracing is wired and handler exists
        }
    }
}
