// Integration tests for config persistence and state migration.

use osg_core::config::PersistentState;
use osg_core::graph::{
    ChannelId, ChannelKind, EffectsConfig, EndpointDescriptor, EqBand, EqConfig, FilterType, Link,
    LinkState, MixerSession, RuntimeState,
};
use osg_core::migration;

// ---------------------------------------------------------------------------
// Test 1: Round-trip — save to TOML, load back, assert equality
// ---------------------------------------------------------------------------

#[test]
fn round_trip_preserves_mixer_session() {
    let mut session = MixerSession::default();
    let mut runtime = RuntimeState::default();

    // Add a channel with EQ and effects.
    let ch_id = ChannelId::new();
    let channel = osg_core::graph::Channel {
        id: ch_id,
        kind: ChannelKind::Source,
        source_type: Default::default(),
        output_node_id: None,
        assigned_apps: Vec::new(),
        auto_app: false,
        allow_app_assignment: true,
    };
    // Transient pipewire_id lives in RuntimeState.
    runtime.set_channel_pipewire_id(ch_id, Some(42));
    session.channels.insert(ch_id, channel);

    // Add a locked link with cell EQ.
    // Note: `pending` is now tracked in RuntimeState, not on Link.
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
        cell_node_id: Some(99), // transient — should be stripped by from_state
    };
    session.links.push(link);

    // Serialize via PersistentState.
    let ps = PersistentState::from_state(session.clone(), &runtime);
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

    // Transient cell_node_id is stripped.
    assert_eq!(loaded_link.cell_node_id, None);
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
// Test 5: Order fields persist across MixerSession serialize/deserialize
// ---------------------------------------------------------------------------

#[test]
fn channel_order_and_mix_order_persist_across_round_trip() {
    let ch_id = ChannelId::new();
    let mix_id = ChannelId::new();
    let mut session = MixerSession::default();
    let runtime = RuntimeState::default();

    session.channel_order = vec![EndpointDescriptor::Channel(ch_id)];
    session.mix_order = vec![EndpointDescriptor::Channel(mix_id)];

    let ps = PersistentState::from_state(session.clone(), &runtime);
    let toml_str = toml::to_string_pretty(&ps).expect("serialize");

    let loaded = migration::migrate(&toml_str).expect("migrate");
    let loaded_session = loaded.into_state();

    assert_eq!(loaded_session.channel_order.len(), 1);
    assert!(
        matches!(loaded_session.channel_order[0], EndpointDescriptor::Channel(id) if id == ch_id)
    );

    assert_eq!(loaded_session.mix_order.len(), 1);
    assert!(
        matches!(loaded_session.mix_order[0], EndpointDescriptor::Channel(id) if id == mix_id)
    );
}

#[test]
fn channel_order_persists_multiple_entries_in_correct_sequence() {
    let ids: Vec<ChannelId> = (0..3).map(|_| ChannelId::new()).collect();
    let mut session = MixerSession::default();
    let runtime = RuntimeState::default();

    session.channel_order = ids
        .iter()
        .map(|&id| EndpointDescriptor::Channel(id))
        .collect();

    let ps = PersistentState::from_state(session, &runtime);
    let toml_str = toml::to_string_pretty(&ps).expect("serialize");
    let loaded_session = migration::migrate(&toml_str).expect("migrate").into_state();

    assert_eq!(loaded_session.channel_order.len(), 3);
    for (i, &id) in ids.iter().enumerate() {
        assert!(
            matches!(loaded_session.channel_order[i], EndpointDescriptor::Channel(loaded_id) if loaded_id == id),
            "channel_order[{i}] mismatch"
        );
    }
}

// ---------------------------------------------------------------------------
// Default PersistentState has current version
// ---------------------------------------------------------------------------

#[test]
fn default_persistent_state_has_current_version() {
    let ps = PersistentState::default();
    let toml_str = toml::to_string_pretty(&ps).expect("serialize default");
    assert!(toml_str.contains(&format!("version = \"{}\"", migration::CURRENT_VERSION)));
}
