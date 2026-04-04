// Tests for SPA POD serialization and validation

use pipewire::spa::{
    param::ParamType,
    pod::{Pod, Value, ValueArray, deserialize::PodDeserializer},
};

fn deserialize_volumes(pod_bytes: &[u8]) -> Option<Vec<f32>> {
    let pod = Pod::from_bytes(pod_bytes)?;
    let value = PodDeserializer::deserialize_any_from(pod.as_bytes())
        .ok()?
        .1;
    match value {
        Value::Object(obj) => obj.properties.iter().find_map(|prop| {
            if let Value::ValueArray(ValueArray::Float(ref floats)) = prop.value {
                Some(floats.clone())
            } else {
                None
            }
        }),
        _ => None,
    }
}

mod pod_stereo_validation {
    use super::*;
    use osg_core::pw::pod::build_node_volume_pod;

    #[test]
    fn stereo_input_preserves_both_channels() {
        let (param_type, pod) = build_node_volume_pod(vec![0.5, 0.8]);
        assert_eq!(param_type, ParamType::Props);
        let vols = deserialize_volumes(pod.bytes()).expect("should deserialize");
        assert_eq!(vols.len(), 2);
        assert!((vols[0] - 0.5).abs() < f32::EPSILON);
        assert!((vols[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn mono_input_expands_to_stereo() {
        let (_, pod) = build_node_volume_pod(vec![0.7]);
        let vols = deserialize_volumes(pod.bytes()).expect("should deserialize");
        assert_eq!(vols.len(), 2);
        assert!((vols[0] - 0.7).abs() < f32::EPSILON);
        assert!((vols[1] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_input_defaults_to_unity_gain() {
        let (_, pod) = build_node_volume_pod(vec![]);
        let vols = deserialize_volumes(pod.bytes()).expect("should deserialize");
        assert_eq!(vols.len(), 2);
        assert!((vols[0] - 1.0).abs() < f32::EPSILON);
        assert!((vols[1] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn multi_channel_input_passed_through() {
        // Surround config (5.1) should pass through unchanged
        let (_, pod) = build_node_volume_pod(vec![0.5, 0.5, 0.3, 0.3, 0.4, 0.6]);
        let vols = deserialize_volumes(pod.bytes()).expect("should deserialize");
        assert_eq!(vols.len(), 6);
    }
}
