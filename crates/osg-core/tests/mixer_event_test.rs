use osg_core::graph::ChannelKind;
use osg_core::graph::events::MixerEvent;
use ulid::Ulid;

#[test]
fn mixer_event_volume_is_serializable() {
    let event = MixerEvent::VolumeChanged {
        node_id: 42,
        channels: vec![0.75, 0.75],
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("volumeChanged"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_create_group_node_serializable() {
    let ulid = Ulid::new();
    let instance_id = Ulid::new();
    let event = MixerEvent::CreateGroupNode {
        name: "Music".to_string(),
        ulid,
        kind: ChannelKind::Sink,
        instance_id,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("createGroupNode"));
    assert!(json.contains("Music"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_request_reconciliation_serializable() {
    let event = MixerEvent::RequestReconciliation;
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("requestReconciliation"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_state_persist_serializable() {
    let event = MixerEvent::StatePersistRequested;
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("statePersistRequested"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_exit_serializable() {
    let event = MixerEvent::Exit;
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("exit"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_create_cell_node_serializable() {
    let instance_id = Ulid::new();
    let event = MixerEvent::CreateCellNode {
        name: "cell-music-headphones".to_string(),
        cell_id: "osg.cell.abc-to-def".to_string(),
        channel_ulid: "abc".to_string(),
        mix_ulid: "def".to_string(),
        instance_id,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("createCellNode"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_update_filter_eq_serializable() {
    use osg_core::graph::EqConfig;
    let event = MixerEvent::UpdateFilterEq {
        filter_key: "cell.abc".to_string(),
        eq: EqConfig::default(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("updateFilterEq"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_accessible_via_crate_events_reexport() {
    // Verify the crate-level re-export works.
    let event: osg_core::events::MixerEvent = osg_core::events::MixerEvent::Exit;
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("exit"));
}

#[test]
fn mixer_event_mute_changed_serializable() {
    let event = MixerEvent::MuteChanged {
        node_id: 7,
        muted: true,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("muteChanged"));
    let back: MixerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn mixer_event_all_link_variants_serializable() {
    let variants = vec![
        MixerEvent::CreatePortLink {
            start_id: 1,
            end_id: 2,
        },
        MixerEvent::CreateNodeLinks {
            start_id: 3,
            end_id: 4,
        },
        MixerEvent::RemovePortLink {
            start_id: 5,
            end_id: 6,
        },
        MixerEvent::RemoveNodeLinks {
            start_id: 7,
            end_id: 8,
        },
    ];
    for event in variants {
        let json = serde_json::to_string(&event).unwrap();
        let back: MixerEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }
}
