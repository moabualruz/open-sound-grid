use osg_core::pw::fft::{FFT_SIZE, FftRingBuffer, SPECTRUM_BINS, SpectrumData, SpectrumHandle};

#[test]
fn fft_dc_signal_energy_in_low_bins() {
    let mut rb = FftRingBuffer::new();
    let dc_signal = vec![0.5_f32; FFT_SIZE];
    assert!(rb.push_samples(&dc_signal));
    let bins = rb.compute_spectrum().expect("should produce spectrum");
    // DC energy should be concentrated in the low-frequency bins
    let low_avg: f32 = bins[..SPECTRUM_BINS / 4].iter().sum::<f32>() / (SPECTRUM_BINS / 4) as f32;
    let high_avg: f32 =
        bins[SPECTRUM_BINS * 3 / 4..].iter().sum::<f32>() / (SPECTRUM_BINS / 4) as f32;
    assert!(
        low_avg > high_avg + 10.0,
        "DC energy should be in low bins: low_avg={low_avg}, high_avg={high_avg}"
    );
}

#[test]
fn fft_sine_1khz_peak_in_correct_bin() {
    let mut rb = FftRingBuffer::new();
    let sample_rate = 48000.0_f32;
    let mut signal = vec![0.0_f32; FFT_SIZE];
    for (i, s) in signal.iter_mut().enumerate() {
        *s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate).sin();
    }
    assert!(rb.push_samples(&signal));
    let bins = rb.compute_spectrum().expect("should produce spectrum");
    let peak_bin = bins
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    // 1kHz in log scale between 20Hz-20kHz is roughly 57% through
    let expected_approx = (SPECTRUM_BINS as f32 * 0.57) as usize;
    let tolerance = SPECTRUM_BINS / 10;
    assert!(
        peak_bin.abs_diff(expected_approx) < tolerance,
        "1kHz peak at bin {peak_bin}, expected near {expected_approx} (+-{tolerance})"
    );
}

#[test]
fn fft_silence_all_bins_near_floor() {
    let mut rb = FftRingBuffer::new();
    let silence = vec![0.0_f32; FFT_SIZE];
    assert!(rb.push_samples(&silence));
    let bins = rb.compute_spectrum().expect("should produce spectrum");
    for (i, &val) in bins.iter().enumerate() {
        assert!(
            val <= -99.0,
            "silence bin {i} should be at noise floor, got {val} dB"
        );
    }
}

#[test]
fn ring_buffer_accumulation_and_full_detection() {
    let mut rb = FftRingBuffer::new();
    let half = vec![0.0_f32; FFT_SIZE / 2];
    assert!(!rb.push_samples(&half), "should not be full at half");
    assert!(
        rb.compute_spectrum().is_none(),
        "should return None when not full"
    );
    assert!(rb.push_samples(&half), "should be full now");
    assert!(rb.compute_spectrum().is_some(), "should compute after full");
    assert!(
        rb.compute_spectrum().is_none(),
        "should be None after reset"
    );
}

#[test]
fn magnitude_scaling_is_db() {
    let mut rb = FftRingBuffer::new();
    let sample_rate = 48000.0_f32;
    let mut signal = vec![0.0_f32; FFT_SIZE];
    for (i, s) in signal.iter_mut().enumerate() {
        *s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate).sin();
    }
    assert!(rb.push_samples(&signal));
    let bins = rb.compute_spectrum().expect("should produce spectrum");
    let peak_val = bins.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    // Full-scale sine with Hann window: expect roughly -6 to 0 dB
    assert!(
        peak_val > -12.0 && peak_val <= 0.0,
        "peak should be in dB scale near 0 dBFS, got {peak_val}"
    );
}

#[test]
fn spectrum_handle_subscribe_flag() {
    let handle = SpectrumHandle::default();
    assert!(!handle.is_subscribed());
    handle.set_subscribed(true);
    assert!(handle.is_subscribed());
    handle.set_subscribed(false);
    assert!(!handle.is_subscribed());
}

#[test]
fn spectrum_handle_publish_and_load() {
    let handle = SpectrumHandle::default();
    let mut data = SpectrumData::default();
    data.bins[0] = -6.0;
    handle.publish(data);
    let loaded = handle.load();
    assert!((loaded.bins[0] - (-6.0)).abs() < f32::EPSILON);
}
