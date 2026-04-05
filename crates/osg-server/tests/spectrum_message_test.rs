use osg_core::pw::fft::SPECTRUM_BINS;
use osg_server::spectrum::SpectrumMessage;

#[test]
fn spectrum_message_round_trips() {
    let message = SpectrumMessage {
        node_id: "channel-a-to-mix-b".to_string(),
        bins: (0..SPECTRUM_BINS).map(|idx| idx as f32 * 0.25).collect(),
    };

    let json = serde_json::to_string(&message).expect("serialize spectrum message");
    let decoded: SpectrumMessage =
        serde_json::from_str(&json).expect("deserialize spectrum message");

    assert_eq!(decoded, message);
    assert_eq!(decoded.bins.len(), SPECTRUM_BINS);
}
