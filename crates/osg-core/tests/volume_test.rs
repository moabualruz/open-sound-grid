// Tests for volume field behavior on Endpoint.
//
// The CODE_REVIEW identified that volumes lack bounds validation (P1-3).
// These tests verify the current observable behavior and document the expected
// range [0.0, 1.5].  They serve as a regression baseline: if bounds clamping
// is ever added, the clamp tests will start asserting the correct behavior.

use osg_core::graph::{ChannelId, Endpoint, EndpointDescriptor, PortKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn channel_endpoint() -> Endpoint {
    let id = ChannelId::new();
    Endpoint::new(EndpointDescriptor::Channel(id))
}

fn ephemeral_endpoint() -> Endpoint {
    Endpoint::new(EndpointDescriptor::EphemeralNode(42, PortKind::Source))
}

// ---------------------------------------------------------------------------
// Test 1: default volume is 1.0 (unity gain)
// ---------------------------------------------------------------------------

#[test]
fn endpoint_default_volume_is_unity() {
    let ep = channel_endpoint();
    assert_eq!(ep.volume, 1.0);
    assert_eq!(ep.volume_left, 1.0);
    assert_eq!(ep.volume_right, 1.0);
}

// ---------------------------------------------------------------------------
// Test 2: volume 0.0 is accepted (silence / minimum)
// ---------------------------------------------------------------------------

#[test]
fn endpoint_accepts_zero_volume() {
    let mut ep = channel_endpoint();
    ep.volume = 0.0;
    ep.volume_left = 0.0;
    ep.volume_right = 0.0;
    assert_eq!(ep.volume, 0.0);
    assert_eq!(ep.volume_left, 0.0);
    assert_eq!(ep.volume_right, 0.0);
}

// ---------------------------------------------------------------------------
// Test 3: volume 1.0 (unity) is accepted
// ---------------------------------------------------------------------------

#[test]
fn endpoint_accepts_unity_volume() {
    let mut ep = channel_endpoint();
    ep.volume = 1.0;
    assert_eq!(ep.volume, 1.0);
}

// ---------------------------------------------------------------------------
// Test 4: volume 1.5 is the documented maximum (150% = headroom boost)
// ---------------------------------------------------------------------------

#[test]
fn endpoint_accepts_maximum_volume_1_5() {
    let mut ep = channel_endpoint();
    ep.volume = 1.5;
    assert_eq!(ep.volume, 1.5);
}

// ---------------------------------------------------------------------------
// Test 5: negative volume — documents current behavior.
// When bounds clamping is implemented, this test should be updated to assert
// the clamped value (0.0) rather than the raw negative.
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_negative_is_not_currently_clamped() {
    let mut ep = channel_endpoint();
    ep.volume = -0.5;
    // Document: no clamping exists yet — negative passes through.
    // If this assertion fails after adding .set_volume(), update to:
    //   assert_eq!(ep.volume, 0.0);
    assert_eq!(
        ep.volume, -0.5,
        "negative volume not clamped — update test when bounds validation is added"
    );
}

// ---------------------------------------------------------------------------
// Test 6: volume above 1.5 — documents current behavior (no clamping yet)
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_above_max_is_not_currently_clamped() {
    let mut ep = channel_endpoint();
    ep.volume = 2.0;
    // Document: no clamping exists yet.
    assert_eq!(
        ep.volume, 2.0,
        "over-max volume not clamped — update test when bounds validation is added"
    );
}

// ---------------------------------------------------------------------------
// Test 7: NaN volume — field assignment does not panic
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_nan_does_not_panic() {
    let mut ep = channel_endpoint();
    ep.volume = f32::NAN;
    // NaN != NaN by IEEE 754.
    assert!(
        ep.volume.is_nan(),
        "expected NaN to be stored (no clamping yet)"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Inf volume — field assignment does not panic
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_inf_does_not_panic() {
    let mut ep = channel_endpoint();
    ep.volume = f32::INFINITY;
    assert!(
        ep.volume.is_infinite(),
        "expected Inf to be stored (no clamping yet)"
    );
}

// ---------------------------------------------------------------------------
// Test 9: stereo volumes are independent
// ---------------------------------------------------------------------------

#[test]
fn endpoint_stereo_volumes_are_independent() {
    let mut ep = ephemeral_endpoint();
    ep.volume_left = 0.3;
    ep.volume_right = 0.8;
    assert_eq!(ep.volume_left, 0.3);
    assert_eq!(ep.volume_right, 0.8);
    // master volume is not automatically updated
    assert_eq!(ep.volume, 1.0);
}

// ---------------------------------------------------------------------------
// Test 10: volume_mixed flag is distinct from the volume fields
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_mixed_flag_is_independent() {
    let mut ep = channel_endpoint();
    assert!(!ep.volume_mixed);
    ep.volume_mixed = true;
    assert!(ep.volume_mixed);
    // toggling the flag does not affect volumes
    assert_eq!(ep.volume, 1.0);
}

// ---------------------------------------------------------------------------
// Test 11: serde round-trip preserves all volume fields
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_fields_survive_serde_round_trip() {
    let mut ep = channel_endpoint();
    ep.volume = 0.7;
    ep.volume_left = 0.6;
    ep.volume_right = 0.8;
    ep.volume_mixed = true;

    let json = serde_json::to_string(&ep).expect("serialize");
    let loaded: Endpoint = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(loaded.volume, 0.7);
    assert_eq!(loaded.volume_left, 0.6);
    assert_eq!(loaded.volume_right, 0.8);
    assert!(loaded.volume_mixed);
}

// ---------------------------------------------------------------------------
// Test 12: volume at exact boundary values
// ---------------------------------------------------------------------------

#[test]
fn endpoint_volume_at_boundary_0_0_is_valid() {
    let mut ep = channel_endpoint();
    ep.set_volume(0.0);
    assert_eq!(ep.volume, 0.0, "0.0 is the lower bound");
}

#[test]
fn endpoint_volume_at_boundary_1_5_is_valid() {
    let mut ep = channel_endpoint();
    ep.set_volume(1.5);
    assert_eq!(ep.volume, 1.5, "1.5 is the upper bound");
}

// ---------------------------------------------------------------------------
// Test 13: set_volume clamps values to [0.0, 1.5]
// ---------------------------------------------------------------------------

#[test]
fn set_volume_clamps_negative_to_zero() {
    let mut ep = channel_endpoint();
    ep.set_volume(-0.5);
    assert_eq!(ep.volume, 0.0);
}

#[test]
fn set_volume_clamps_above_max_to_1_5() {
    let mut ep = channel_endpoint();
    ep.set_volume(2.0);
    assert_eq!(ep.volume, 1.5);
}

// ---------------------------------------------------------------------------
// Test 14: set_stereo_volume clamps and computes average
// ---------------------------------------------------------------------------

#[test]
fn set_stereo_volume_clamps_and_averages() {
    let mut ep = channel_endpoint();
    ep.set_stereo_volume(-0.1, 2.0);
    assert_eq!(ep.volume_left, 0.0);
    assert_eq!(ep.volume_right, 1.5);
    assert_eq!(ep.volume, 0.75); // (0.0 + 1.5) / 2
}

#[test]
fn set_stereo_volume_normal_values() {
    let mut ep = channel_endpoint();
    ep.set_stereo_volume(0.3, 0.8);
    assert_eq!(ep.volume_left, 0.3);
    assert_eq!(ep.volume_right, 0.8);
    assert!((ep.volume - 0.55).abs() < f32::EPSILON);
}
