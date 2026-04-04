// Integration tests for config persistence and state migration.

use osg_core::config::PersistentState;
use osg_core::graph::{
    ChannelId, ChannelKind, EffectsConfig, EndpointDescriptor, EqBand, EqConfig, FilterType, Link,
    LinkState, MixerSession,
};
use osg_core::migration;

// ---------------------------------------------------------------------------
// Test 1: Round-trip — save to TOML, load back, assert equality
// ---------------------------------------------------------------------------

#[test]
fn round_trip_preserves_mixer_session() {
    let mut session = MixerSession::default();

    // Add a channel with EQ and effects.
    let ch_id = ChannelId::new();
    let mut channel = osg_core::graph::Channel {
        id: ch_id,
        kind: ChannelKind::Source,
        source_type: Default::default(),
        output_node_id: None,
        assigned_apps: Vec::new(),
        auto_app: false,
        allow_app_assignment: true,
        pipewire_id: None,
        pending: false,
    };
    // Transient fields should not affect serialization.
    channel.pipewire_id = Some(42);
    session.channels.insert(ch_id, channel);

    // Add a locked link with cell EQ.
    let link = Link {
        start: EndpointDescriptor::Channel(ch_id),
        end: EndpointDescriptor::Channel(ChannelId::new()),
        state: LinkState::ConnectedLocked,
        cell_volume: 0.8,
        cell_volume_left: 0.8,
        cell_volume_right: 0.75,
        cell_eq: EqConfig {
            enabled: true,
            bands: vec![EqBand {
                enabled: true,
                filter_type: FilterType::HighShelf,
                frequency: 8000.0,
                gain: 3.0,
                q: 1.0,
            }],
        },
        cell_effects: EffectsConfig::default(),
        cell_node_id: Some(99), // transient — should be skipped
        pending: true,          // transient — should be skipped
    };
    session.links.push(link);

    // Serialize via PersistentState.
    let ps = PersistentState::from_state(session.clone());
    let toml_str = toml::to_string_pretty(&ps).expect("serialize");

    // Deserialize back via migration.
    let loaded = migration::migrate(&toml_str).expect("migrate");
    let loaded_session = loaded.into_state();

    // Channel survived.
    assert!(loaded_session.channels.contains_key(&ch_id));

    // Locked link survived (from_state retains locked links).
    assert_eq!(loaded_session.links.len(), 1);
    let loaded_link = &loaded_session.links[0];
    assert_eq!(loaded_link.cell_volume, 0.8);
    assert_eq!(loaded_link.cell_volume_right, 0.75);
    assert_eq!(loaded_link.cell_eq.bands.len(), 1);
    assert_eq!(loaded_link.cell_eq.bands[0].frequency, 8000.0);

    // Transient fields reset to defaults.
    assert_eq!(loaded_link.cell_node_id, None);
    assert!(!loaded_link.pending);
}

// ---------------------------------------------------------------------------
// Test 2: Missing fields — old TOML without new fields gets defaults
// ---------------------------------------------------------------------------

#[test]
fn missing_fields_get_serde_defaults() {
    // Minimal valid PersistentState TOML — missing eq, effects, channel_order, etc.
    let minimal_toml = r#"
version = "0.2.0"

[state]
active_sources = []
active_sinks = []
endpoints = []
links = []

[state.persistent_nodes]
[state.apps]
[state.devices]
[state.channels]
"#;

    let result = migration::migrate(minimal_toml).expect("migrate minimal");
    let session = result.into_state();

    // Fields that were missing should have their defaults.
    assert!(session.channel_order.is_empty());
    assert!(session.mix_order.is_empty());
    assert!(session.links.is_empty());
}

// ---------------------------------------------------------------------------
// Test 3: Corrupt TOML — graceful fallback to default
// ---------------------------------------------------------------------------

#[test]
fn corrupt_toml_returns_default() {
    let garbage = "{{{{not valid toml at all!!! @@@ }}}}";

    let result = migration::migrate(garbage).expect("should not error");
    let session = result.into_state();

    // Should be a fresh default session.
    assert!(session.channels.is_empty());
    assert!(session.links.is_empty());
    assert!(session.active_sources.is_empty());
}

#[test]
fn empty_string_returns_default() {
    let result = migration::migrate("").expect("should not error");
    let session = result.into_state();
    assert!(session.channels.is_empty());
}

// ---------------------------------------------------------------------------
// Test 4: Version mismatch — old version triggers migration path
// ---------------------------------------------------------------------------

#[test]
fn old_version_triggers_migration() {
    // State saved by version 0.1.0 — missing effects fields but otherwise valid.
    let old_toml = r#"
version = "0.1.0"

[state]
active_sources = []
active_sinks = []
endpoints = []
links = []

[state.persistent_nodes]
[state.apps]
[state.devices]
[state.channels]
"#;

    let result = migration::migrate(old_toml).expect("migrate old version");
    let session = result.into_state();

    // Migration succeeded — we get a valid session with defaults for new fields.
    assert!(session.channels.is_empty());
    assert!(session.channel_order.is_empty());
}

#[test]
fn unknown_future_version_returns_default() {
    let future_toml = r#"
version = "99.0.0"

[state]
active_sources = []
active_sinks = []
endpoints = []
links = []

[state.persistent_nodes]
[state.apps]
[state.devices]
[state.channels]
"#;

    let result = migration::migrate(future_toml).expect("should not error");
    let session = result.into_state();

    // Unknown version falls back to defaults (not the data in the file).
    assert!(session.channels.is_empty());
}

#[test]
fn missing_version_field_returns_default() {
    let no_version = r#"
[state]
active_sources = []
active_sinks = []
endpoints = []
links = []

[state.persistent_nodes]
[state.apps]
[state.devices]
[state.channels]
"#;

    let result = migration::migrate(no_version).expect("should not error");
    let session = result.into_state();
    assert!(session.channels.is_empty());
}

// ---------------------------------------------------------------------------
// Test 5: Default PersistentState has current version
// ---------------------------------------------------------------------------

#[test]
fn default_persistent_state_has_current_version() {
    let ps = PersistentState::default();
    let toml_str = toml::to_string_pretty(&ps).expect("serialize default");
    assert!(toml_str.contains(&format!("version = \"{}\"", migration::CURRENT_VERSION)));
}
