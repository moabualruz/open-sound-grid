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
