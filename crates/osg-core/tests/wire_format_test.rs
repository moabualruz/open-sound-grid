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
// Spectrum subscribe/unsubscribe wire format
// ---------------------------------------------------------------------------

#[test]
fn spectrum_subscribe_message_deserializes() {
    let msg = r#"{"subscribe":["osg.filter.abc","osg.filter.def"]}"#;
    let val: serde_json::Value = serde_json::from_str(msg).expect("valid JSON");
    let keys = val["subscribe"].as_array().expect("subscribe array");
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].as_str().unwrap(), "osg.filter.abc");
    assert_eq!(keys[1].as_str().unwrap(), "osg.filter.def");
}

#[test]
fn spectrum_unsubscribe_message_deserializes() {
    let msg = r#"{"unsubscribe":["osg.filter.abc"]}"#;
    let val: serde_json::Value = serde_json::from_str(msg).expect("valid JSON");
    let keys = val["unsubscribe"].as_array().expect("unsubscribe array");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].as_str().unwrap(), "osg.filter.abc");
}

#[test]
fn spectrum_response_wire_format() {
    use osg_core::pw::fft::SPECTRUM_BINS;
    // Verify the expected output shape: { "spectra": { "nodeId": { "left": [...], "right": [...] } } }
    let left = vec![0.0_f32; SPECTRUM_BINS];
    let right = vec![-100.0_f32; SPECTRUM_BINS];
    let payload = serde_json::json!({
        "spectra": {
            "osg.filter.abc": {
                "left": left,
                "right": right,
            }
        }
    });
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("deserialize");
    let spectra = parsed["spectra"]["osg.filter.abc"].as_object().expect("node object");
    assert_eq!(
        spectra["left"].as_array().expect("left array").len(),
        SPECTRUM_BINS
    );
    assert_eq!(
        spectra["right"].as_array().expect("right array").len(),
        SPECTRUM_BINS
    );
}
