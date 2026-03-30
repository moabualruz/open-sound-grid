//! Synchronous PA mutation helpers — module load/unload, volume, mute, and move.
//!
//! Split from `introspect.rs` to keep files under the 500-line limit.
//! Same lock → operation → wait → unlock contract as the list helpers.

use std::sync::{Arc, Mutex};

use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::volume::{ChannelVolumes, Volume};
use tracing::instrument;

use crate::error::{OsgError, Result};

use super::connection::PulseConnection;

// ---------------------------------------------------------------------------
// Module load / unload
// ---------------------------------------------------------------------------

/// Load a PA module synchronously. Returns the module index on success.
///
/// Follows the same lock → operation → wait → unlock contract as the list helpers.
/// On PipeWire/PA the callback delivers `PA_INVALID_INDEX` (u32::MAX) on failure.
#[instrument(skip(conn))]
pub fn load_module_sync(conn: &mut PulseConnection, name: &str, args: &str) -> Result<u32> {
    tracing::debug!(module_name = %name, args = %args, "loading PA module via libpulse");

    let result: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let result_clone = Arc::clone(&result);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .load_module(name, args, move |idx| {
            tracing::trace!(module_idx = idx, "load_module callback fired");
            *result_clone.lock().unwrap() = Some(idx);
            // SAFETY: mainloop outlives this callback; signal() is the intended
            // mechanism for waking a locked mainloop from within a callback.
            unsafe { (*ml_ptr).signal(false) };
        });

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let idx = result
        .lock()
        .unwrap()
        .ok_or_else(|| OsgError::ModuleLoadFailed(name.to_string()))?;

    // PA_INVALID_INDEX (u32::MAX) means the server rejected the load.
    if idx == u32::MAX {
        tracing::error!(module_name = %name, "load_module returned PA_INVALID_INDEX");
        return Err(OsgError::ModuleLoadFailed(name.to_string()));
    }

    tracing::debug!(module_name = %name, module_id = idx, "PA module loaded");
    Ok(idx)
}

/// Unload a PA module synchronously.
#[instrument(skip(conn))]
pub fn unload_module_sync(conn: &mut PulseConnection, module_id: u32) -> Result<()> {
    tracing::debug!(module_id = module_id, "unloading PA module via libpulse");

    let success: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let success_clone = Arc::clone(&success);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .unload_module(module_id, move |ok| {
            tracing::trace!(
                module_id = module_id,
                ok = ok,
                "unload_module callback fired"
            );
            *success_clone.lock().unwrap() = ok;
            unsafe { (*ml_ptr).signal(false) };
        });

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if *success.lock().unwrap() {
        tracing::debug!(module_id = module_id, "PA module unloaded");
        Ok(())
    } else {
        tracing::error!(module_id = module_id, "unload_module returned failure");
        Err(OsgError::PulseAudio(format!(
            "unload module {module_id} failed"
        )))
    }
}

// ---------------------------------------------------------------------------
// Sink-input volume / mute / move
// ---------------------------------------------------------------------------

/// Build a mono `ChannelVolumes` with all channels set to `volume` (0.0–1.0+).
/// Convert a linear slider position (0.0–1.0) to a PA Volume using a cubic
/// perceptual curve. This matches PulseAudio's `pa_sw_volume_from_linear()`:
/// perceived loudness scales roughly with the cube root of power, so we apply
/// a cubic mapping to make the slider feel natural.
///
/// Without this, 50% slider = 50% PA_VOLUME_NORM = ~25% perceived loudness.
/// With cubic: 50% slider ≈ 50% perceived loudness.
fn linear_to_pa_volume(linear: f32) -> Volume {
    if linear <= 0.0 {
        return Volume::MUTED;
    }
    if linear >= 1.0 {
        return Volume::NORMAL;
    }
    // PA cubic curve: pa_volume = PA_VOLUME_NORM * cbrt(linear)^3
    // But that's just linear again. The ACTUAL PA curve from pa_sw_volume_from_linear is:
    // volume = PA_VOLUME_NORM * (linear)^(1/3) for the "software" curve.
    // This means small slider values produce larger PA volumes = more audible at low end.
    let curved = (linear as f64).cbrt();
    let raw = (curved * Volume::NORMAL.0 as f64) as u32;
    Volume(raw.min(Volume::NORMAL.0))
}

fn make_channel_volumes(volume: f32) -> ChannelVolumes {
    let vol = linear_to_pa_volume(volume);
    let mut cv = ChannelVolumes::default();
    // Must set 2 channels (stereo) — loopbacks and null-sinks are stereo by default.
    // Setting only 1 channel causes libpulse/PipeWire to silently reject the volume change.
    cv.set(2, vol);
    cv
}

/// Set the volume of a sink-input synchronously.
#[instrument(skip(conn))]
pub fn set_sink_input_volume_sync(conn: &mut PulseConnection, idx: u32, volume: f32) -> Result<()> {
    tracing::debug!(
        sink_input_idx = idx,
        volume = volume,
        "setting sink-input volume via libpulse"
    );

    let cv = make_channel_volumes(volume);
    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn.context_mut().introspect().set_sink_input_volume(
        idx,
        &cv,
        Some(Box::new(move |success| {
            tracing::trace!(
                sink_input_idx = idx,
                success = success,
                "set_sink_input_volume callback fired"
            );
            *done_clone.lock().unwrap() = success;
            unsafe { (*ml_ptr).signal(false) };
        })),
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(
            sink_input_idx = idx,
            "set_sink_input_volume returned failure"
        );
    }
    Ok(())
}

/// Build stereo `ChannelVolumes` with independent left/right values.
fn make_stereo_channel_volumes(left: f32, right: f32) -> ChannelVolumes {
    let vol_left = linear_to_pa_volume(left);
    let vol_right = linear_to_pa_volume(right);
    let mut cv = ChannelVolumes::default();
    cv.set(2, vol_left); // sets both channels to left initially
    // Override channel 1 (right) with the right volume
    let channels = cv.get_mut();
    channels[1] = vol_right;
    cv
}

/// Set independent L/R stereo volume on a sink-input synchronously.
#[instrument(skip(conn))]
pub fn set_sink_input_stereo_volume_sync(
    conn: &mut PulseConnection,
    idx: u32,
    left: f32,
    right: f32,
) -> Result<()> {
    tracing::debug!(
        sink_input_idx = idx, left, right,
        "setting sink-input stereo volume via libpulse"
    );

    let cv = make_stereo_channel_volumes(left, right);
    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn.context_mut().introspect().set_sink_input_volume(
        idx,
        &cv,
        Some(Box::new(move |success| {
            tracing::trace!(
                sink_input_idx = idx, success,
                "set_sink_input_volume (stereo) callback fired"
            );
            *done_clone.lock().unwrap() = success;
            unsafe { (*ml_ptr).signal(false) };
        })),
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(sink_input_idx = idx, "set_sink_input_volume (stereo) returned failure");
    }
    Ok(())
}

/// Set the mute state of a sink-input synchronously.
#[instrument(skip(conn))]
pub fn set_sink_input_mute_sync(conn: &mut PulseConnection, idx: u32, mute: bool) -> Result<()> {
    tracing::debug!(
        sink_input_idx = idx,
        mute = mute,
        "setting sink-input mute via libpulse"
    );

    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn.context_mut().introspect().set_sink_input_mute(
        idx,
        mute,
        Some(Box::new(move |success| {
            tracing::trace!(
                sink_input_idx = idx,
                success = success,
                "set_sink_input_mute callback fired"
            );
            *done_clone.lock().unwrap() = success;
            unsafe { (*ml_ptr).signal(false) };
        })),
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(sink_input_idx = idx, "set_sink_input_mute returned failure");
    }
    Ok(())
}

/// Move a sink-input to a different sink synchronously (by sink name).
#[instrument(skip(conn))]
pub fn move_sink_input_sync(conn: &mut PulseConnection, idx: u32, sink_name: &str) -> Result<()> {
    tracing::debug!(sink_input_idx = idx, sink_name = %sink_name, "moving sink-input via libpulse");

    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn.context_mut().introspect().move_sink_input_by_name(
        idx,
        sink_name,
        Some(Box::new(move |success| {
            tracing::trace!(
                sink_input_idx = idx,
                success = success,
                "move_sink_input callback fired"
            );
            *done_clone.lock().unwrap() = success;
            unsafe { (*ml_ptr).signal(false) };
        })),
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(sink_input_idx = idx, sink_name = %sink_name, "move_sink_input returned failure");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sink volume / mute (by name)
// ---------------------------------------------------------------------------

/// Set the volume of a sink by name synchronously.
#[instrument(skip(conn))]
pub fn set_sink_volume_by_name_sync(
    conn: &mut PulseConnection,
    sink_name: &str,
    volume: f32,
) -> Result<()> {
    tracing::debug!(sink_name = %sink_name, volume = volume, "setting sink volume by name via libpulse");

    let cv = make_channel_volumes(volume);
    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();
    let sink_name_owned = sink_name.to_owned();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .set_sink_volume_by_name(
            sink_name,
            &cv,
            Some(Box::new(move |success| {
                tracing::trace!(sink_name = %sink_name_owned, success = success, "set_sink_volume_by_name callback fired");
                *done_clone.lock().unwrap() = success;
                unsafe { (*ml_ptr).signal(false) };
            })),
        );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(sink_name = %sink_name, "set_sink_volume_by_name returned failure");
    }
    Ok(())
}

/// Set the mute state of a sink by name synchronously.
#[instrument(skip(conn))]
pub fn set_sink_mute_by_name_sync(
    conn: &mut PulseConnection,
    sink_name: &str,
    mute: bool,
) -> Result<()> {
    tracing::debug!(sink_name = %sink_name, mute = mute, "setting sink mute by name via libpulse");

    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();
    let sink_name_owned = sink_name.to_owned();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .set_sink_mute_by_name(
            sink_name,
            mute,
            Some(Box::new(move |success| {
                tracing::trace!(sink_name = %sink_name_owned, success = success, "set_sink_mute_by_name callback fired");
                *done_clone.lock().unwrap() = success;
                unsafe { (*ml_ptr).signal(false) };
            })),
        );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(sink_name = %sink_name, "set_sink_mute_by_name returned failure");
    }
    Ok(())
}
