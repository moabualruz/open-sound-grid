use osg_core::graph::{EqBand, EqConfig, FilterType};

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

#[test]
fn eq_config_default_is_enabled_with_no_bands() {
    let eq = EqConfig::default();
    assert!(eq.enabled);
    assert!(eq.bands.is_empty());
}

#[test]
fn eq_band_default_is_peaking_at_1khz() {
    let band = EqBand::default();
    assert!(band.enabled);
    assert_eq!(band.filter_type, FilterType::Peaking);
    assert!((band.frequency - 1000.0).abs() < f32::EPSILON);
    assert!((band.gain).abs() < f32::EPSILON);
    assert!((band.q - 0.707).abs() < 0.001);
}

// ---------------------------------------------------------------------------
// Serialization round-trip (TOML, matching state.toml persistence)
// ---------------------------------------------------------------------------

#[test]
fn eq_config_toml_round_trip() {
    let config = EqConfig {
        enabled: true,
        bands: vec![
            EqBand {
                enabled: true,
                filter_type: FilterType::Peaking,
                frequency: 1000.0,
                gain: 3.5,
                q: 1.4,
            },
            EqBand {
                enabled: false,
                filter_type: FilterType::HighPass,
                frequency: 80.0,
                gain: 0.0,
                q: 0.707,
            },
            EqBand {
                enabled: true,
                filter_type: FilterType::LowShelf,
                frequency: 200.0,
                gain: -2.0,
                q: 0.5,
            },
        ],
    };
    let toml_str = toml::to_string(&config).expect("serialize");
    let restored: EqConfig = toml::from_str(&toml_str).expect("deserialize");

    assert_eq!(restored.enabled, config.enabled);
    assert_eq!(restored.bands.len(), 3);
    assert_eq!(restored.bands[0].filter_type, FilterType::Peaking);
    assert!((restored.bands[0].gain - 3.5).abs() < f32::EPSILON);
    assert_eq!(restored.bands[1].filter_type, FilterType::HighPass);
    assert!(!restored.bands[1].enabled);
    assert_eq!(restored.bands[2].filter_type, FilterType::LowShelf);
}

// ---------------------------------------------------------------------------
// JSON round-trip (matching WebSocket wire format)
// ---------------------------------------------------------------------------

#[test]
fn eq_config_json_round_trip_camel_case() {
    let config = EqConfig {
        enabled: true,
        bands: vec![EqBand {
            enabled: true,
            filter_type: FilterType::Notch,
            frequency: 400.0,
            gain: -6.0,
            q: 5.0,
        }],
    };
    let json = serde_json::to_string(&config).expect("serialize");
    // Wire format uses camelCase
    assert!(json.contains("filterType"));
    assert!(json.contains("\"notch\""));

    let restored: EqConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.bands[0].filter_type, FilterType::Notch);
    assert!((restored.bands[0].frequency - 400.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// All filter types serialize correctly
// ---------------------------------------------------------------------------

#[test]
fn all_filter_types_json_round_trip() {
    let types = [
        FilterType::Peaking,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::Notch,
    ];
    for ft in &types {
        let json = serde_json::to_string(ft).expect("serialize");
        let restored: FilterType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&restored, ft);
    }
}

// ---------------------------------------------------------------------------
// Backward compatibility: old state.toml without eq field deserializes fine
// ---------------------------------------------------------------------------

#[test]
fn endpoint_without_eq_field_deserializes_with_default() {
    // Simulate an old state.toml endpoint entry that has no `eq` field.
    // ChannelId is a ULID — use a valid 26-char ULID string.
    let json = r#"{
        "descriptor": {"channel": "01KN1CSN9NCZDP0J5Z830MAX26"},
        "isPlaceholder": false,
        "displayName": "Test",
        "customName": null,
        "iconName": "audio-card",
        "details": [],
        "volume": 0.8,
        "volumeLeft": 0.8,
        "volumeRight": 0.8,
        "volumeMixed": false,
        "volumeLockedMuted": "unmutedUnlocked",
        "visible": true
    }"#;
    let ep: osg_core::graph::Endpoint = serde_json::from_str(json).expect("deserialize");
    assert!(ep.eq.enabled);
    assert!(ep.eq.bands.is_empty());
}

// ---------------------------------------------------------------------------
// Backward compatibility: old Link without cell_eq deserializes fine
// ---------------------------------------------------------------------------

#[test]
fn link_without_cell_eq_field_deserializes_with_default() {
    let json = r#"{
        "start": {"channel": "01KN1CSN9NCZDP0J5Z830MAX26"},
        "end": {"channel": "01KN1CSN9NCZDP0J5Z830MAX27"},
        "state": "connectedLocked",
        "cellVolume": 1.0,
        "cellVolumeLeft": 1.0,
        "cellVolumeRight": 1.0
    }"#;
    let link: osg_core::graph::Link = serde_json::from_str(json).expect("deserialize");
    assert!(link.cell_eq.enabled);
    assert!(link.cell_eq.bands.is_empty());
}
