//! PipeWire real-time process callback for `OsgFilter`.
//!
//! Extracted from `filter.rs` for file-size compliance.
//! Runs on the PW RT thread — no allocations, no locks.

use super::filter_handle::{CompiledEq, SAMPLE_RATE};
use super::{CallbackData, FilterHandle};
use crate::pw::biquad::BiquadState;
use crate::pw::effects_dsp::{
    apply_boost, apply_compressor, apply_de_esser, apply_gate, apply_limiter, apply_smart_volume,
    apply_spatial,
};
use crate::pw::fft::{FftRingBuffer, SpectrumData};

/// Process a block of mono audio through the EQ biquad cascade.
pub fn process_block(
    input: &[f32],
    output: &mut [f32],
    eq: &CompiledEq,
    states: &mut [BiquadState],
) -> f32 {
    let mut peak: f32 = 0.0;
    for (i, &sample) in input.iter().enumerate() {
        let mut s = sample;
        for (band_idx, (coeffs, enabled)) in eq.bands.iter().enumerate() {
            if *enabled && let Some(state) = states.get_mut(band_idx) {
                s = state.process(s, coeffs);
            }
        }
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
        output[i] = s;
    }
    peak
}

struct SpectrumInput<'a> {
    left: Option<&'a [f32]>,
    right: Option<&'a [f32]>,
}

fn publish_spectrum_if_ready(
    handle: &FilterHandle,
    fft: &mut FftRingBuffer,
    input: SpectrumInput<'_>,
) {
    if !handle.spectrum().is_subscribed() {
        return;
    }

    let mut ready = false;

    if let (Some(left), Some(right)) = (input.left, input.right) {
        // Collapse stereo to mono for one spectrum payload per filter node.
        for (&left, &right) in left.iter().zip(right.iter()) {
            ready |= fft.push_sample((left + right) * 0.5);
        }
    } else if let Some(left) = input.left {
        ready = fft.push_samples(left);
    } else if let Some(right) = input.right {
        ready = fft.push_samples(right);
    }

    if ready && let Some(bins) = fft.compute_spectrum() {
        handle.spectrum().publish(SpectrumData { bins });
    }
}

/// Process callback — runs on PW real-time thread.
/// Reads stereo input, applies EQ cascade, volume gain, mute, computes peaks, writes output.
///
/// # SAFETY
/// This function is called by PipeWire's RT thread via the `process` function pointer
/// registered in `OsgFilter::new()`. Safety guarantees:
///
/// 1. **`data` pointer lifetime**: The `CallbackData` is allocated via `Box::into_raw` in
///    `OsgFilter::new()` and reclaimed exclusively in `OsgFilter::drop()`. The `data`
///    pointer is valid for the entire lifetime of the filter.
///
/// 2. **Buffer validity**: `pw_filter_get_dsp_buffer` returns pointers to PW-managed
///    buffers valid for this callback invocation. Null checks guard output pointers.
///
/// 3. **`position` pointer**: PW always provides a valid `spa_io_position` during
///    process callbacks. Null check is a defensive guard.
///
/// 4. **No shared mutation**: The callback reads params via `FilterHandle` (lock-free
///    atomics/ArcSwap) and writes only to `d.states_*` and `d.env_*` exclusively owned
///    by this `CallbackData`.
#[allow(unsafe_code, clippy::too_many_lines)]
pub(super) unsafe extern "C" fn on_process(
    data: *mut std::os::raw::c_void,
    position: *mut libspa_sys::spa_io_position,
) {
    let d = &mut *(data as *mut CallbackData);

    if position.is_null() {
        return;
    }
    let n_samples = (*position).clock.duration as u32;
    if n_samples == 0 {
        return;
    }

    let in_l = pipewire_sys::pw_filter_get_dsp_buffer(d.in_port_l, n_samples) as *const f32;
    let in_r = pipewire_sys::pw_filter_get_dsp_buffer(d.in_port_r, n_samples) as *const f32;
    let out_l = pipewire_sys::pw_filter_get_dsp_buffer(d.out_port_l, n_samples) as *mut f32;
    let out_r = pipewire_sys::pw_filter_get_dsp_buffer(d.out_port_r, n_samples) as *mut f32;

    let n = n_samples as usize;
    let muted = d.handle.is_muted();
    let bypassed = d.handle.is_bypassed();
    let (vol_l, vol_r) = d.handle.volume();

    let mut peak_l: f32 = 0.0;
    let mut peak_r: f32 = 0.0;

    // Track whether each channel has live audio (for spatial crossfeed).
    let mut l_has_signal = false;
    let mut r_has_signal = false;

    // Always write output — silence if no input or muted, to prevent graph stalls.
    if !out_l.is_null() {
        let out_slice_l = std::slice::from_raw_parts_mut(out_l, n);
        if muted || in_l.is_null() {
            out_slice_l.fill(0.0);
        } else {
            let in_slice_l = std::slice::from_raw_parts(in_l, n);
            let fx = d.handle.load_effects();
            if bypassed {
                out_slice_l.copy_from_slice(in_slice_l);
            } else {
                let eq = d.handle.load_eq();
                process_block(in_slice_l, out_slice_l, &eq, &mut d.states_l);
            }
            apply_gate(out_slice_l, &fx.gate, &mut d.env_l, SAMPLE_RATE);
            apply_compressor(out_slice_l, &fx.compressor, &mut d.env_l, SAMPLE_RATE);
            apply_de_esser(out_slice_l, &fx.de_esser);
            apply_limiter(out_slice_l, &fx.limiter);
            apply_boost(out_slice_l, fx.boost);
            apply_smart_volume(out_slice_l, &fx.smart_volume, &mut d.env_l, SAMPLE_RATE);
            l_has_signal = true;
        }
    }
    if !out_r.is_null() {
        let out_slice_r = std::slice::from_raw_parts_mut(out_r, n);
        if muted || in_r.is_null() {
            out_slice_r.fill(0.0);
        } else {
            let in_slice_r = std::slice::from_raw_parts(in_r, n);
            let fx = d.handle.load_effects();
            if bypassed {
                out_slice_r.copy_from_slice(in_slice_r);
            } else {
                let eq = d.handle.load_eq();
                process_block(in_slice_r, out_slice_r, &eq, &mut d.states_r);
            }
            apply_gate(out_slice_r, &fx.gate, &mut d.env_r, SAMPLE_RATE);
            apply_compressor(out_slice_r, &fx.compressor, &mut d.env_r, SAMPLE_RATE);
            apply_de_esser(out_slice_r, &fx.de_esser);
            apply_limiter(out_slice_r, &fx.limiter);
            apply_boost(out_slice_r, fx.boost);
            apply_smart_volume(out_slice_r, &fx.smart_volume, &mut d.env_r, SAMPLE_RATE);
            r_has_signal = true;
        }
    }

    // Spatial audio — operates on both channels simultaneously, after per-channel effects.
    if l_has_signal && r_has_signal && !out_l.is_null() && !out_r.is_null() {
        let fx = d.handle.load_effects();
        let out_slice_l = std::slice::from_raw_parts_mut(out_l, n);
        let out_slice_r = std::slice::from_raw_parts_mut(out_r, n);
        apply_spatial(out_slice_l, out_slice_r, &fx.spatial);
    }

    // Volume gain and peak measurement (after spatial).
    if !out_l.is_null() {
        let out_slice_l = std::slice::from_raw_parts_mut(out_l, n);
        if l_has_signal && (vol_l - 1.0).abs() > f32::EPSILON {
            for s in out_slice_l.iter_mut() {
                *s *= vol_l;
            }
        }
        peak_l = out_slice_l.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
    }
    if !out_r.is_null() {
        let out_slice_r = std::slice::from_raw_parts_mut(out_r, n);
        if r_has_signal && (vol_r - 1.0).abs() > f32::EPSILON {
            for s in out_slice_r.iter_mut() {
                *s *= vol_r;
            }
        }
        peak_r = out_slice_r.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
    }
    d.handle.store_peaks(peak_l, peak_r);

    let spectrum_input = SpectrumInput {
        left: (l_has_signal && !out_l.is_null()).then(|| std::slice::from_raw_parts(out_l, n)),
        right: (r_has_signal && !out_r.is_null()).then(|| std::slice::from_raw_parts(out_r, n)),
    };
    publish_spectrum_if_ready(&d.handle, &mut d.fft, spectrum_input);
}
