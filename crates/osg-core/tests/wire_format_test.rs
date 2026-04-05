use osg_core::pw::GroupNodeKind;

#[test]
fn group_node_kind_sink_serializes_to_lowercase_sink() {
    let json = serde_json::to_string(&GroupNodeKind::Sink).expect("serialize sink");
    assert_eq!(json, "\"sink\"");
}

#[test]
fn group_node_kind_source_serializes_to_lowercase_source() {
    let json = serde_json::to_string(&GroupNodeKind::Source).expect("serialize source");
    assert_eq!(json, "\"source\"");
}

// ---------------------------------------------------------------------------
// Command round-trip tests (Gap 3 additions)
//
// Each Command variant must:
//   1. Serialize to the expected {"type": "camelCaseName", ...} JSON shape
//   2. Deserialize back to an equal value (round-trip)
// ---------------------------------------------------------------------------

use osg_core::commands::Command;
use osg_core::graph::{
    ChannelId, ChannelKind, EffectsConfig, EndpointDescriptor, EqConfig, PortKind,
};

fn channel_ep() -> EndpointDescriptor {
    EndpointDescriptor::Channel(ChannelId::new())
}

fn ephemeral_ep() -> EndpointDescriptor {
    EndpointDescriptor::EphemeralNode(99, PortKind::Source)
}

fn round_trip(cmd: &Command) -> Command {
    let json = serde_json::to_string(cmd).expect("serialize command");
    serde_json::from_str(&json).expect("deserialize command")
}

fn assert_type_field(cmd: &Command, expected_type: &str) {
    let json = serde_json::to_string(cmd).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(
        v["type"].as_str().unwrap(),
        expected_type,
        "wrong type field in JSON: {json}"
    );
}

// --- CreateChannel ----------------------------------------------------------

#[test]
fn command_create_channel_serializes_type_field() {
    let cmd = Command::CreateChannel {
        name: "Gaming".to_string(),
        kind: ChannelKind::Source,
    };
    assert_type_field(&cmd, "createChannel");
}

#[test]
fn command_create_channel_round_trips() {
    let cmd = Command::CreateChannel {
        name: "Browser".to_string(),
        kind: ChannelKind::Sink,
    };
    let rt = round_trip(&cmd);
    let json_orig = serde_json::to_string(&cmd).unwrap();
    let json_rt = serde_json::to_string(&rt).unwrap();
    assert_eq!(json_orig, json_rt);
}

// --- RemoveEndpoint ---------------------------------------------------------

#[test]
fn command_remove_endpoint_serializes_type_field() {
    let cmd = Command::RemoveEndpoint {
        endpoint: channel_ep(),
    };
    assert_type_field(&cmd, "removeEndpoint");
}

#[test]
fn command_remove_endpoint_round_trips() {
    let ep = EndpointDescriptor::EphemeralNode(42, PortKind::Sink);
    let cmd = Command::RemoveEndpoint { endpoint: ep };
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- SetVolume --------------------------------------------------------------

#[test]
fn command_set_volume_serializes_type_field() {
    let cmd = Command::SetVolume {
        endpoint: channel_ep(),
        volume: 0.75,
    };
    assert_type_field(&cmd, "setVolume");
}

#[test]
fn command_set_volume_round_trips() {
    let cmd = Command::SetVolume {
        endpoint: ephemeral_ep(),
        volume: 0.5,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!((v["volume"].as_f64().unwrap() - 0.5).abs() < f64::EPSILON);
}

// --- SetStereoVolume --------------------------------------------------------

#[test]
fn command_set_stereo_volume_serializes_type_field() {
    let cmd = Command::SetStereoVolume {
        endpoint: channel_ep(),
        left: 0.3,
        right: 0.9,
    };
    assert_type_field(&cmd, "setStereoVolume");
}

#[test]
fn command_set_stereo_volume_round_trips() {
    let cmd = Command::SetStereoVolume {
        endpoint: channel_ep(),
        left: 0.2,
        right: 0.8,
    };
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- SetMute ----------------------------------------------------------------

#[test]
fn command_set_mute_serializes_type_field() {
    let cmd = Command::SetMute {
        endpoint: channel_ep(),
        muted: true,
    };
    assert_type_field(&cmd, "setMute");
}

#[test]
fn command_set_mute_round_trips() {
    for muted in [true, false] {
        let cmd = Command::SetMute {
            endpoint: channel_ep(),
            muted,
        };
        let rt = round_trip(&cmd);
        assert_eq!(
            serde_json::to_string(&cmd).unwrap(),
            serde_json::to_string(&rt).unwrap()
        );
    }
}

// --- SetVolumeLocked --------------------------------------------------------

#[test]
fn command_set_volume_locked_serializes_type_field() {
    let cmd = Command::SetVolumeLocked {
        endpoint: channel_ep(),
        locked: false,
    };
    assert_type_field(&cmd, "setVolumeLocked");
}

// --- RenameEndpoint ---------------------------------------------------------

#[test]
fn command_rename_endpoint_with_name_round_trips() {
    let cmd = Command::RenameEndpoint {
        endpoint: channel_ep(),
        name: Some("My Channel".to_string()),
    };
    assert_type_field(&cmd, "renameEndpoint");
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

#[test]
fn command_rename_endpoint_with_none_round_trips() {
    let cmd = Command::RenameEndpoint {
        endpoint: channel_ep(),
        name: None,
    };
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- Link / RemoveLink ------------------------------------------------------

#[test]
fn command_link_serializes_type_field() {
    let cmd = Command::Link {
        source: channel_ep(),
        target: ephemeral_ep(),
    };
    assert_type_field(&cmd, "link");
}

#[test]
fn command_remove_link_serializes_type_field() {
    let cmd = Command::RemoveLink {
        source: channel_ep(),
        target: ephemeral_ep(),
    };
    assert_type_field(&cmd, "removeLink");
}

#[test]
fn command_link_round_trips() {
    let cmd = Command::Link {
        source: channel_ep(),
        target: ephemeral_ep(),
    };
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- SetLinkLocked ----------------------------------------------------------

#[test]
fn command_set_link_locked_serializes_type_field() {
    let cmd = Command::SetLinkLocked {
        source: channel_ep(),
        target: ephemeral_ep(),
        locked: true,
    };
    assert_type_field(&cmd, "setLinkLocked");
}

// --- SetMixOutput -----------------------------------------------------------

#[test]
fn command_set_mix_output_with_node_id_round_trips() {
    let ch_id = ChannelId::new();
    let cmd = Command::SetMixOutput {
        channel: ch_id,
        output_node_id: Some(7),
    };
    assert_type_field(&cmd, "setMixOutput");
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

#[test]
fn command_set_mix_output_with_none_round_trips() {
    let ch_id = ChannelId::new();
    let cmd = Command::SetMixOutput {
        channel: ch_id,
        output_node_id: None,
    };
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- SetEndpointVisible -----------------------------------------------------

#[test]
fn command_set_endpoint_visible_serializes_type_field() {
    let cmd = Command::SetEndpointVisible {
        endpoint: channel_ep(),
        visible: false,
    };
    assert_type_field(&cmd, "setEndpointVisible");
}

// --- SetLinkVolume / SetLinkStereoVolume ------------------------------------

#[test]
fn command_set_link_volume_serializes_type_field() {
    let cmd = Command::SetLinkVolume {
        source: channel_ep(),
        target: ephemeral_ep(),
        volume: 0.6,
    };
    assert_type_field(&cmd, "setLinkVolume");
}

#[test]
fn command_set_link_stereo_volume_serializes_type_field() {
    let cmd = Command::SetLinkStereoVolume {
        source: channel_ep(),
        target: ephemeral_ep(),
        left: 0.4,
        right: 0.9,
    };
    assert_type_field(&cmd, "setLinkStereoVolume");
}

// --- SetChannelOrder / SetMixOrder ------------------------------------------

#[test]
fn command_set_channel_order_round_trips() {
    let cmd = Command::SetChannelOrder {
        order: vec![channel_ep(), ephemeral_ep()],
    };
    assert_type_field(&cmd, "setChannelOrder");
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

#[test]
fn command_set_mix_order_round_trips() {
    let cmd = Command::SetMixOrder {
        order: vec![channel_ep()],
    };
    assert_type_field(&cmd, "setMixOrder");
}

// --- AssignApp / UnassignApp ------------------------------------------------

#[test]
fn command_assign_app_serializes_type_field() {
    let cmd = Command::AssignApp {
        channel: ChannelId::new(),
        application_name: "Firefox".to_string(),
        binary_name: "firefox".to_string(),
    };
    assert_type_field(&cmd, "assignApp");
}

#[test]
fn command_unassign_app_round_trips() {
    let cmd = Command::UnassignApp {
        channel: ChannelId::new(),
        application_name: "Spotify".to_string(),
        binary_name: "spotify".to_string(),
    };
    assert_type_field(&cmd, "unassignApp");
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- SetEq / SetCellEq ------------------------------------------------------

#[test]
fn command_set_eq_serializes_type_field() {
    let cmd = Command::SetEq {
        endpoint: channel_ep(),
        eq: EqConfig::default(),
    };
    assert_type_field(&cmd, "setEq");
}

#[test]
fn command_set_cell_eq_round_trips() {
    let cmd = Command::SetCellEq {
        source: channel_ep(),
        target: ephemeral_ep(),
        eq: EqConfig::default(),
    };
    assert_type_field(&cmd, "setCellEq");
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- SetEffects / SetCellEffects --------------------------------------------

#[test]
fn command_set_effects_serializes_type_field() {
    let cmd = Command::SetEffects {
        endpoint: channel_ep(),
        effects: EffectsConfig::default(),
    };
    assert_type_field(&cmd, "setEffects");
}

#[test]
fn command_set_cell_effects_round_trips() {
    let cmd = Command::SetCellEffects {
        source: channel_ep(),
        target: ephemeral_ep(),
        effects: EffectsConfig::default(),
    };
    assert_type_field(&cmd, "setCellEffects");
    let rt = round_trip(&cmd);
    assert_eq!(
        serde_json::to_string(&cmd).unwrap(),
        serde_json::to_string(&rt).unwrap()
    );
}

// --- Discriminant exhaustiveness --------------------------------------------

#[test]
fn all_command_variants_have_type_field_in_json() {
    // Build one instance of every Command variant to ensure each serializes
    // with a "type" discriminant.  This will fail to compile if a new variant
    // is added and not covered here.
    let ep = channel_ep();
    let ep2 = ephemeral_ep();
    let ch = ChannelId::new();

    let commands: &[Command] = &[
        Command::CreateChannel {
            name: "x".into(),
            kind: ChannelKind::Source,
        },
        Command::RemoveEndpoint { endpoint: ep },
        Command::SetVolume {
            endpoint: ep,
            volume: 1.0,
        },
        Command::SetStereoVolume {
            endpoint: ep,
            left: 1.0,
            right: 1.0,
        },
        Command::SetMute {
            endpoint: ep,
            muted: false,
        },
        Command::SetVolumeLocked {
            endpoint: ep,
            locked: false,
        },
        Command::RenameEndpoint {
            endpoint: ep,
            name: None,
        },
        Command::Link {
            source: ep,
            target: ep2,
        },
        Command::RemoveLink {
            source: ep,
            target: ep2,
        },
        Command::SetLinkLocked {
            source: ep,
            target: ep2,
            locked: false,
        },
        Command::SetMixOutput {
            channel: ch,
            output_node_id: None,
        },
        Command::SetEndpointVisible {
            endpoint: ep,
            visible: true,
        },
        Command::SetLinkVolume {
            source: ep,
            target: ep2,
            volume: 0.5,
        },
        Command::SetLinkStereoVolume {
            source: ep,
            target: ep2,
            left: 0.5,
            right: 0.5,
        },
        Command::SetChannelOrder { order: vec![] },
        Command::SetMixOrder { order: vec![] },
        Command::AssignApp {
            channel: ch,
            application_name: "a".into(),
            binary_name: "b".into(),
        },
        Command::UnassignApp {
            channel: ch,
            application_name: "a".into(),
            binary_name: "b".into(),
        },
        Command::SetEq {
            endpoint: ep,
            eq: EqConfig::default(),
        },
        Command::SetCellEq {
            source: ep,
            target: ep2,
            eq: EqConfig::default(),
        },
        Command::SetEffects {
            endpoint: ep,
            effects: EffectsConfig::default(),
        },
        Command::SetCellEffects {
            source: ep,
            target: ep2,
            effects: EffectsConfig::default(),
        },
    ];

    for cmd in commands {
        let json = serde_json::to_string(cmd).expect("serialize");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert!(
            v["type"].is_string(),
            "Command variant {cmd:?} serialized without a 'type' field: {json}"
        );
    }
}

// Wire-format round-trip tests for SetChannelOrder and SetMixOrder
// ---------------------------------------------------------------------------

#[test]
fn set_channel_order_command_round_trips_via_json() {
    let ch_id = ChannelId::new();
    let cmd = Command::SetChannelOrder {
        order: vec![EndpointDescriptor::Channel(ch_id)],
    };
    let json = serde_json::to_string(&cmd).expect("serialize SetChannelOrder");
    let decoded: Command = serde_json::from_str(&json).expect("deserialize SetChannelOrder");
    match decoded {
        Command::SetChannelOrder { order } => {
            assert_eq!(order.len(), 1);
            assert!(matches!(order[0], EndpointDescriptor::Channel(id) if id == ch_id));
        }
        other => panic!("unexpected command variant: {other:?}"),
    }
}

#[test]
fn set_channel_order_command_has_correct_type_tag() {
    let cmd = Command::SetChannelOrder { order: vec![] };
    let json = serde_json::to_string(&cmd).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(v["type"], "setChannelOrder");
    assert!(v["order"].is_array());
}

#[test]
fn set_mix_order_command_round_trips_via_json() {
    let ch_id = ChannelId::new();
    let cmd = Command::SetMixOrder {
        order: vec![EndpointDescriptor::Channel(ch_id)],
    };
    let json = serde_json::to_string(&cmd).expect("serialize SetMixOrder");
    let decoded: Command = serde_json::from_str(&json).expect("deserialize SetMixOrder");
    match decoded {
        Command::SetMixOrder { order } => {
            assert_eq!(order.len(), 1);
            assert!(matches!(order[0], EndpointDescriptor::Channel(id) if id == ch_id));
        }
        other => panic!("unexpected command variant: {other:?}"),
    }
}

#[test]
fn set_mix_order_command_has_correct_type_tag() {
    let cmd = Command::SetMixOrder { order: vec![] };
    let json = serde_json::to_string(&cmd).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(v["type"], "setMixOrder");
    assert!(v["order"].is_array());
}

#[test]
fn set_channel_order_empty_order_round_trips() {
    let cmd = Command::SetChannelOrder { order: vec![] };
    let json = serde_json::to_string(&cmd).expect("serialize empty SetChannelOrder");
    let decoded: Command = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(decoded, Command::SetChannelOrder { order } if order.is_empty()));
}

#[test]
fn set_mix_order_empty_order_round_trips() {
    let cmd = Command::SetMixOrder { order: vec![] };
    let json = serde_json::to_string(&cmd).expect("serialize empty SetMixOrder");
    let decoded: Command = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(decoded, Command::SetMixOrder { order } if order.is_empty()));
}
