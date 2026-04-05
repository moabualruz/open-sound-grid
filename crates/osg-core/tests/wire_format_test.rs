use osg_core::commands::Command;
use osg_core::graph::EndpointDescriptor;
use osg_core::graph::ChannelId;
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
