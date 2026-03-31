use osg_core::graph::FilterType;
use osg_core::pw::biquad::{BiquadState, compute_coefficients};

const SAMPLE_RATE: f32 = 48000.0;

// ---------------------------------------------------------------------------
// Coefficient computation — basic sanity checks
// ---------------------------------------------------------------------------

#[test]
fn peaking_zero_gain_is_unity() {
    let c = compute_coefficients(FilterType::Peaking, 1000.0, 0.0, 1.0, SAMPLE_RATE);
    // Zero gain peaking filter should be identity: b0/a0 ≈ 1, b1/a1 equal, b2/a2 equal
    let ratio = c.b0 / c.a0;
    assert!(
        (ratio - 1.0).abs() < 0.001,
        "b0/a0 should be ~1 for zero-gain peaking, got {ratio}"
    );
}

#[test]
fn peaking_positive_gain_boosts() {
    let c = compute_coefficients(FilterType::Peaking, 1000.0, 6.0, 1.0, SAMPLE_RATE);
    // At center frequency, the magnitude should be > 1 (boost)
    let mag_db = magnitude_db_at(&c, 1000.0, SAMPLE_RATE);
    assert!(
        mag_db > 5.0 && mag_db < 7.0,
        "expected ~6dB boost at center freq, got {mag_db:.2}dB"
    );
}

#[test]
fn peaking_negative_gain_cuts() {
    let c = compute_coefficients(FilterType::Peaking, 1000.0, -6.0, 1.0, SAMPLE_RATE);
    let mag_db = magnitude_db_at(&c, 1000.0, SAMPLE_RATE);
    assert!(
        mag_db < -5.0 && mag_db > -7.0,
        "expected ~-6dB cut at center freq, got {mag_db:.2}dB"
    );
}

#[test]
fn highpass_attenuates_low_frequencies() {
    let c = compute_coefficients(FilterType::HighPass, 1000.0, 0.0, 0.707, SAMPLE_RATE);
    let low = magnitude_db_at(&c, 100.0, SAMPLE_RATE);
    let high = magnitude_db_at(&c, 10000.0, SAMPLE_RATE);
    assert!(
        low < -10.0,
        "100Hz should be heavily attenuated by 1kHz highpass, got {low:.2}dB"
    );
    assert!(
        high > -1.0,
        "10kHz should pass through 1kHz highpass, got {high:.2}dB"
    );
}

#[test]
fn lowpass_attenuates_high_frequencies() {
    let c = compute_coefficients(FilterType::LowPass, 1000.0, 0.0, 0.707, SAMPLE_RATE);
    let low = magnitude_db_at(&c, 100.0, SAMPLE_RATE);
    let high = magnitude_db_at(&c, 10000.0, SAMPLE_RATE);
    assert!(
        low > -1.0,
        "100Hz should pass through 1kHz lowpass, got {low:.2}dB"
    );
    assert!(
        high < -10.0,
        "10kHz should be attenuated by 1kHz lowpass, got {high:.2}dB"
    );
}

#[test]
fn notch_cuts_at_center() {
    let c = compute_coefficients(FilterType::Notch, 1000.0, 0.0, 5.0, SAMPLE_RATE);
    let at_center = magnitude_db_at(&c, 1000.0, SAMPLE_RATE);
    let off_center = magnitude_db_at(&c, 5000.0, SAMPLE_RATE);
    assert!(
        at_center < -20.0,
        "notch should deeply cut center freq, got {at_center:.2}dB"
    );
    assert!(
        off_center > -1.0,
        "notch should pass off-center, got {off_center:.2}dB"
    );
}

#[test]
fn low_shelf_boosts_below_frequency() {
    let c = compute_coefficients(FilterType::LowShelf, 500.0, 6.0, 0.707, SAMPLE_RATE);
    let below = magnitude_db_at(&c, 50.0, SAMPLE_RATE);
    let above = magnitude_db_at(&c, 5000.0, SAMPLE_RATE);
    assert!(
        below > 4.0,
        "50Hz should be boosted by low shelf at 500Hz, got {below:.2}dB"
    );
    assert!(
        above.abs() < 1.0,
        "5kHz should be flat with low shelf at 500Hz, got {above:.2}dB"
    );
}

#[test]
fn high_shelf_boosts_above_frequency() {
    let c = compute_coefficients(FilterType::HighShelf, 2000.0, 6.0, 0.707, SAMPLE_RATE);
    let below = magnitude_db_at(&c, 200.0, SAMPLE_RATE);
    let above = magnitude_db_at(&c, 15000.0, SAMPLE_RATE);
    assert!(
        below.abs() < 1.0,
        "200Hz should be flat with high shelf at 2kHz, got {below:.2}dB"
    );
    assert!(
        above > 4.0,
        "15kHz should be boosted by high shelf at 2kHz, got {above:.2}dB"
    );
}

// ---------------------------------------------------------------------------
// BiquadState — sample-by-sample processing
// ---------------------------------------------------------------------------

#[test]
fn biquad_state_passthrough_on_unity_filter() {
    let c = compute_coefficients(FilterType::Peaking, 1000.0, 0.0, 1.0, SAMPLE_RATE);
    let mut state = BiquadState::new();
    // Feed impulse
    let out = state.process(1.0, &c);
    // For a unity filter, output should be very close to input
    assert!(
        (out - 1.0).abs() < 0.01,
        "unity filter should pass impulse through, got {out}"
    );
}

#[test]
fn biquad_state_reset_clears_history() {
    let c = compute_coefficients(FilterType::Peaking, 1000.0, 6.0, 1.0, SAMPLE_RATE);
    let mut state = BiquadState::new();
    // Process some samples
    for _ in 0..100 {
        state.process(0.5, &c);
    }
    state.reset();
    // After reset, internal state should be zero — next output depends only on input
    let out = state.process(0.0, &c);
    assert!(
        out.abs() < f32::EPSILON,
        "after reset, zero input should produce zero output, got {out}"
    );
}

// ---------------------------------------------------------------------------
// Helper: evaluate magnitude response in dB at a given frequency
// ---------------------------------------------------------------------------

fn magnitude_db_at(
    c: &osg_core::pw::biquad::Coefficients,
    freq: f32,
    sample_rate: f32,
) -> f32 {
    let w = std::f32::consts::TAU * freq / sample_rate;
    let cos_w = w.cos();
    let cos_2w = (2.0 * w).cos();
    let sin_w = w.sin();
    let sin_2w = (2.0 * w).sin();

    let num_re = c.b0 + c.b1 * cos_w + c.b2 * cos_2w;
    let num_im = -(c.b1 * sin_w + c.b2 * sin_2w);
    let den_re = c.a0 + c.a1 * cos_w + c.a2 * cos_2w;
    let den_im = -(c.a1 * sin_w + c.a2 * sin_2w);

    let num_mag_sq = num_re * num_re + num_im * num_im;
    let den_mag_sq = den_re * den_re + den_im * den_im;

    if den_mag_sq == 0.0 {
        return 0.0;
    }
    10.0 * (num_mag_sq / den_mag_sq).log10()
}
