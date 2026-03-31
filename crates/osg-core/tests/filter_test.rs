use osg_core::graph::{EqBand, EqConfig, FilterType};
use osg_core::pw::biquad::BiquadState;
use osg_core::pw::filter::{CompiledEq, FilterHandle, pack_peaks, process_block, unpack_peaks};

// ---------------------------------------------------------------------------
// Peak packing / unpacking
// ---------------------------------------------------------------------------

#[test]
fn pack_unpack_peaks_round_trips() {
    let (l, r) = (0.75_f32, 0.25_f32);
    let packed = pack_peaks(l, r);
    let (ul, ur) = unpack_peaks(packed);
    assert!((ul - l).abs() < f32::EPSILON);
    assert!((ur - r).abs() < f32::EPSILON);
}

#[test]
fn pack_unpack_peaks_zero() {
    let packed = pack_peaks(0.0, 0.0);
    let (l, r) = unpack_peaks(packed);
    assert_eq!(l, 0.0);
    assert_eq!(r, 0.0);
}

// ---------------------------------------------------------------------------
// FilterHandle — lock-free EQ parameter passing
// ---------------------------------------------------------------------------

#[test]
fn filter_handle_default_eq_is_empty() {
    let handle = FilterHandle::new();
    let eq = handle.load_eq();
    assert!(eq.bands.is_empty());
}

#[test]
fn filter_handle_set_eq_updates_compiled() {
    let handle = FilterHandle::new();
    handle.set_eq(&EqConfig {
        enabled: true,
        bands: vec![EqBand {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 6.0,
            q: 1.0,
        }],
    });
    let eq = handle.load_eq();
    assert_eq!(eq.bands.len(), 1);
    assert!(eq.bands[0].1); // enabled
}

#[test]
fn filter_handle_disabled_eq_marks_bands_disabled() {
    let handle = FilterHandle::new();
    handle.set_eq(&EqConfig {
        enabled: false,
        bands: vec![EqBand {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 6.0,
            q: 1.0,
        }],
    });
    let eq = handle.load_eq();
    assert!(!eq.bands[0].1); // disabled because config.enabled = false
}

#[test]
fn filter_handle_peaks_default_zero() {
    let handle = FilterHandle::new();
    let (l, r) = handle.peak();
    assert_eq!(l, 0.0);
    assert_eq!(r, 0.0);
}

#[test]
fn filter_handle_store_and_read_peaks() {
    let handle = FilterHandle::new();
    handle.store_peaks(0.9, 0.3);
    let (l, r) = handle.peak();
    assert!((l - 0.9).abs() < f32::EPSILON);
    assert!((r - 0.3).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// process_block — EQ cascade on audio buffer
// ---------------------------------------------------------------------------

#[test]
fn process_block_passthrough_with_no_bands() {
    let eq = CompiledEq::empty();
    let mut states = vec![BiquadState::default(); 10];
    let input = vec![0.5_f32; 128];
    let mut output = vec![0.0_f32; 128];
    let peak = process_block(&input, &mut output, &eq, &mut states);
    // No bands → passthrough
    assert!((output[0] - 0.5).abs() < f32::EPSILON);
    assert!((peak - 0.5).abs() < f32::EPSILON);
}

#[test]
fn process_block_with_peaking_boost_increases_signal() {
    let eq = CompiledEq::from_config(&EqConfig {
        enabled: true,
        bands: vec![EqBand {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 12.0,
            q: 1.0,
        }],
    });
    let mut states = vec![BiquadState::default(); 10];
    // Generate a 1kHz sine wave at 48kHz sample rate
    let input: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin() * 0.1)
        .collect();
    let mut output = vec![0.0_f32; 1024];
    let _peak = process_block(&input, &mut output, &eq, &mut states);

    // After settling, the output should be louder than input (12dB boost ≈ 4x)
    let input_rms: f32 = (input[512..].iter().map(|s| s * s).sum::<f32>()
        / (input.len() - 512) as f32)
        .sqrt();
    let output_rms: f32 = (output[512..].iter().map(|s| s * s).sum::<f32>()
        / (output.len() - 512) as f32)
        .sqrt();
    assert!(
        output_rms > input_rms * 2.0,
        "12dB boost should significantly increase signal: input_rms={input_rms:.4}, output_rms={output_rms:.4}"
    );
}

#[test]
fn process_block_disabled_band_is_passthrough() {
    let eq = CompiledEq::from_config(&EqConfig {
        enabled: true,
        bands: vec![EqBand {
            enabled: false, // disabled
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 12.0,
            q: 1.0,
        }],
    });
    let mut states = vec![BiquadState::default(); 10];
    let input = vec![0.5_f32; 128];
    let mut output = vec![0.0_f32; 128];
    process_block(&input, &mut output, &eq, &mut states);
    assert!((output[64] - 0.5).abs() < f32::EPSILON);
}
