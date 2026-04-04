// Tests for the event translator (MixerEvent → ToPipewireMessage).

use osg_core::graph::ChannelKind;
use osg_core::graph::events::MixerEvent;
use osg_core::pw::ToPipewireMessage;
use osg_core::routing::event_translator;
use ulid::Ulid;

#[test]
fn volume_changed_translates_to_node_volume() {
    let event = MixerEvent::VolumeChanged {
        node_id: 42,
        channels: vec![0.5, 0.8],
    };
    let msgs = event_translator::translate(&event);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], ToPipewireMessage::NodeVolume(42, vec![0.5, 0.8]));
}

#[test]
fn mute_changed_translates_to_node_mute() {
    let event = MixerEvent::MuteChanged {
        node_id: 7,
        muted: true,
    };
    let msgs = event_translator::translate(&event);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], ToPipewireMessage::NodeMute(7, true));
}

#[test]
fn create_group_node_translates_correctly() {
    let ulid = Ulid::new();
    let instance_id = Ulid::new();
    let event = MixerEvent::CreateGroupNode {
        name: "Monitor".to_string(),
        ulid,
        kind: ChannelKind::Sink,
        instance_id,
    };
    let msgs = event_translator::translate(&event);
    assert_eq!(msgs.len(), 1);
    assert_eq!(
        msgs[0],
        ToPipewireMessage::CreateGroupNode(
            "Monitor".to_string(),
            ulid,
            osg_core::pw::GroupNodeKind::Sink,
            instance_id,
        )
    );
}

#[test]
fn request_reconciliation_translates_to_update() {
    let event = MixerEvent::RequestReconciliation;
    let msgs = event_translator::translate(&event);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], ToPipewireMessage::Update);
}

#[test]
fn state_persist_requested_produces_no_pw_message() {
    let event = MixerEvent::StatePersistRequested;
    let msgs = event_translator::translate(&event);
    assert!(msgs.is_empty());
}

#[test]
fn translate_all_handles_multiple_events() {
    let events = vec![
        MixerEvent::VolumeChanged {
            node_id: 1,
            channels: vec![0.5],
        },
        MixerEvent::MuteChanged {
            node_id: 2,
            muted: false,
        },
        MixerEvent::RemoveNodeLinks {
            start_id: 10,
            end_id: 20,
        },
        MixerEvent::StatePersistRequested,
        MixerEvent::Exit,
    ];
    let msgs = event_translator::translate_all(&events);
    // StatePersistRequested produces 0, rest produce 1 each
    assert_eq!(msgs.len(), 4);
    assert_eq!(msgs[0], ToPipewireMessage::NodeVolume(1, vec![0.5]));
    assert_eq!(msgs[1], ToPipewireMessage::NodeMute(2, false));
    assert_eq!(
        msgs[2],
        ToPipewireMessage::RemoveNodeLinks {
            start_id: 10,
            end_id: 20,
        }
    );
    assert_eq!(msgs[3], ToPipewireMessage::Exit);
}

#[test]
fn create_cell_node_translates_all_fields() {
    let instance_id = Ulid::new();
    let event = MixerEvent::CreateCellNode {
        name: "Music→Monitor".to_string(),
        cell_id: "osg.cell.abc-to-xyz".to_string(),
        channel_ulid: "abc".to_string(),
        mix_ulid: "xyz".to_string(),
        instance_id,
    };
    let msgs = event_translator::translate(&event);
    assert_eq!(msgs.len(), 1);
    assert_eq!(
        msgs[0],
        ToPipewireMessage::CreateCellNode {
            name: "Music→Monitor".to_string(),
            cell_id: "osg.cell.abc-to-xyz".to_string(),
            channel_ulid: "abc".to_string(),
            mix_ulid: "xyz".to_string(),
            instance_id,
        }
    );
}

#[test]
fn create_staging_sink_translates() {
    let instance_id = Ulid::new();
    let event = MixerEvent::CreateStagingSink { instance_id };
    let msgs = event_translator::translate(&event);
    assert_eq!(msgs.len(), 1);
    assert_eq!(
        msgs[0],
        ToPipewireMessage::CreateStagingSink { instance_id }
    );
}
