//! Synchronous introspect helpers wrapping the libpulse threaded-mainloop pattern.
//!
//! Each helper follows the same structure:
//! 1. Lock mainloop
//! 2. Start the async introspect operation — the callback runs on the PA thread
//! 3. Wait for the callback to signal the mainloop (End or Error branch)
//! 4. Unlock mainloop
//! 5. Return the collected results
//!
//! The raw-pointer signal pattern is identical to what `connection.rs` already
//! does for the state callback and is safe under the threaded-mainloop contract:
//! the mainloop outlives every operation, and `signal(false)` is explicitly
//! designed to be called from within a locked callback.

use std::sync::{Arc, Mutex};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::proplist::properties;
use libpulse_binding::volume::{ChannelVolumes, Volume};
use tracing::instrument;

use crate::error::{OsgError, Result};
use crate::plugin::api::{HardwareInput, HardwareOutput};

use super::connection::PulseConnection;

// ---------------------------------------------------------------------------
// Internal transfer types (only what we need from each PA struct)
// ---------------------------------------------------------------------------

/// Minimal data extracted from a PA SinkInfo.
#[derive(Debug)]
struct RawSink {
    index: u32,
    name: String,
    description: String,
}

/// Minimal data extracted from a PA SourceInfo.
#[derive(Debug)]
struct RawSource {
    index: u32,
    name: String,
    description: String,
    /// `true` when this source is a monitor of another sink.
    is_monitor: bool,
}

/// Minimal data extracted from a PA SinkInputInfo.
#[derive(Debug)]
struct RawSinkInput {
    index: u32,
    app_name: Option<String>,
    app_binary: Option<String>,
    icon_name: Option<String>,
    media_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// List all PA sinks synchronously via the introspect API.
///
/// Must be called from the plugin thread.  The mainloop must NOT be locked
/// by the caller — this function handles the lock/wait/unlock cycle internally.
#[instrument(skip(conn))]
pub fn list_sinks_sync(conn: &mut PulseConnection) -> Vec<HardwareOutput> {
    tracing::debug!("listing sinks via libpulse introspect API");

    let results: Arc<Mutex<Vec<RawSink>>> = Arc::new(Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    // SAFETY: mainloop outlives this function; signal() is the intended
    // mechanism for waking a waiting mainloop from within a locked callback.
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    conn.context_mut()
        .introspect()
        .get_sink_info_list(move |list_result| match list_result {
            ListResult::Item(info) => {
                let name = info.name.as_deref().unwrap_or("").to_owned();
                let description = info.description.as_deref().unwrap_or("").to_owned();
                tracing::trace!(index = info.index, name = %name, "got sink from introspect");
                results_clone
                    .lock()
                    .unwrap()
                    .push(RawSink { index: info.index, name, description });
            }
            ListResult::End => {
                tracing::trace!("sink list complete — signalling mainloop");
                // SAFETY: see comment above.
                unsafe { (*ml_ptr).signal(false) };
            }
            ListResult::Error => {
                tracing::warn!("introspect get_sink_info_list returned error");
                unsafe { (*ml_ptr).signal(false) };
            }
        });

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let raw = Arc::try_unwrap(results)
        .expect("no other Arc holders after wait")
        .into_inner()
        .unwrap();

    tracing::debug!(total_raw = raw.len(), "raw sinks received from introspect");

    raw.into_iter()
        .filter(|s| !should_exclude_sink(&s.name))
        .map(|s| HardwareOutput {
            id: s.index,
            name: s.description.clone(),
            description: s.description,
            device_id: s.name,
        })
        .collect()
}

/// List all PA sources synchronously via the introspect API.
///
/// Monitor sources (those whose `monitor_of_sink` is `Some`) are excluded —
/// this is more reliable than the previous `.monitor` suffix check.
#[instrument(skip(conn))]
pub fn list_sources_sync(conn: &mut PulseConnection) -> Vec<HardwareInput> {
    tracing::debug!("listing sources via libpulse introspect API");

    let results: Arc<Mutex<Vec<RawSource>>> = Arc::new(Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    conn.context_mut()
        .introspect()
        .get_source_info_list(move |list_result| match list_result {
            ListResult::Item(info) => {
                let name = info.name.as_deref().unwrap_or("").to_owned();
                let description = info.description.as_deref().unwrap_or("").to_owned();
                let is_monitor = info.monitor_of_sink.is_some();
                tracing::trace!(
                    index = info.index,
                    name = %name,
                    is_monitor = is_monitor,
                    "got source from introspect"
                );
                results_clone.lock().unwrap().push(RawSource {
                    index: info.index,
                    name,
                    description,
                    is_monitor,
                });
            }
            ListResult::End => {
                tracing::trace!("source list complete — signalling mainloop");
                unsafe { (*ml_ptr).signal(false) };
            }
            ListResult::Error => {
                tracing::warn!("introspect get_source_info_list returned error");
                unsafe { (*ml_ptr).signal(false) };
            }
        });

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let raw = Arc::try_unwrap(results)
        .expect("no other Arc holders after wait")
        .into_inner()
        .unwrap();

    tracing::debug!(total_raw = raw.len(), "raw sources received from introspect");

    raw.into_iter()
        .filter(|s| {
            if s.is_monitor {
                tracing::trace!(name = %s.name, "excluding monitor source");
                return false;
            }
            true
        })
        .map(|s| HardwareInput {
            id: s.index,
            name: s.description.clone(),
            description: s.description,
        })
        .collect()
}

/// List all PA sink-inputs synchronously via the introspect API.
///
/// Returns raw entries; stable-ID assignment is left to `AppDetector` so the
/// existing ID-continuity logic is unchanged.
#[instrument(skip(conn))]
pub fn list_sink_inputs_sync(conn: &mut PulseConnection) -> Vec<RawSinkInputResult> {
    tracing::debug!("listing sink-inputs via libpulse introspect API");

    let results: Arc<Mutex<Vec<RawSinkInput>>> = Arc::new(Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    conn.context_mut()
        .introspect()
        .get_sink_input_info_list(move |list_result| match list_result {
            ListResult::Item(info) => {
                let app_name = info.proplist.get_str(properties::APPLICATION_NAME);
                let app_binary =
                    info.proplist.get_str(properties::APPLICATION_PROCESS_BINARY);
                let icon_name =
                    info.proplist.get_str(properties::APPLICATION_ICON_NAME);
                let media_name = info.proplist.get_str(properties::MEDIA_NAME);

                tracing::trace!(
                    index = info.index,
                    app_name = ?app_name,
                    media_name = ?media_name,
                    "got sink-input from introspect"
                );

                results_clone.lock().unwrap().push(RawSinkInput {
                    index: info.index,
                    app_name,
                    app_binary,
                    icon_name,
                    media_name,
                });
            }
            ListResult::End => {
                tracing::trace!("sink-input list complete — signalling mainloop");
                unsafe { (*ml_ptr).signal(false) };
            }
            ListResult::Error => {
                tracing::warn!("introspect get_sink_input_info_list returned error");
                unsafe { (*ml_ptr).signal(false) };
            }
        });

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let raw = Arc::try_unwrap(results)
        .expect("no other Arc holders after wait")
        .into_inner()
        .unwrap();

    tracing::debug!(total_raw = raw.len(), "raw sink-inputs received from introspect");

    raw.into_iter()
        .filter_map(|entry| {
            // Must have an application name.
            let name = match entry.app_name {
                Some(n) => n,
                None => {
                    tracing::debug!(
                        index = entry.index,
                        reason = "no application.name property",
                        "filtering out sink-input"
                    );
                    return None;
                }
            };

            // Skip loopback streams.
            if let Some(ref media) = entry.media_name {
                if media.to_lowercase().contains("loopback") {
                    tracing::debug!(
                        index = entry.index,
                        app_name = %name,
                        media_name = %media,
                        reason = "loopback media stream",
                        "filtering out sink-input"
                    );
                    return None;
                }
            }

            Some(RawSinkInputResult {
                stream_index: entry.index,
                name,
                binary: entry.app_binary.unwrap_or_default(),
                icon_name: entry.icon_name,
            })
        })
        .collect()
}

/// Filtered, consumer-facing result from `list_sink_inputs_sync`.
pub struct RawSinkInputResult {
    pub stream_index: u32,
    pub name: String,
    pub binary: String,
    pub icon_name: Option<String>,
}

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
            tracing::trace!(module_id = module_id, ok = ok, "unload_module callback fired");
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
        Err(OsgError::PulseAudio(format!("unload module {module_id} failed")))
    }
}

// ---------------------------------------------------------------------------
// Sink-input volume / mute / move
// ---------------------------------------------------------------------------

/// Build a mono `ChannelVolumes` with all channels set to `volume` (0.0–1.0+).
fn make_channel_volumes(volume: f32) -> ChannelVolumes {
    let raw = (volume * Volume::NORMAL.0 as f32) as u32;
    let vol = Volume(raw);
    let mut cv = ChannelVolumes::default();
    cv.set(1, vol);
    cv
}

/// Set the volume of a sink-input synchronously.
#[instrument(skip(conn))]
pub fn set_sink_input_volume_sync(
    conn: &mut PulseConnection,
    idx: u32,
    volume: f32,
) -> Result<()> {
    tracing::debug!(sink_input_idx = idx, volume = volume, "setting sink-input volume via libpulse");

    let cv = make_channel_volumes(volume);
    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .set_sink_input_volume(
            idx,
            &cv,
            Some(Box::new(move |success| {
                tracing::trace!(sink_input_idx = idx, success = success, "set_sink_input_volume callback fired");
                *done_clone.lock().unwrap() = success;
                unsafe { (*ml_ptr).signal(false) };
            })),
        );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    if !*done.lock().unwrap() {
        tracing::warn!(sink_input_idx = idx, "set_sink_input_volume returned failure");
    }
    Ok(())
}

/// Set the mute state of a sink-input synchronously.
#[instrument(skip(conn))]
pub fn set_sink_input_mute_sync(
    conn: &mut PulseConnection,
    idx: u32,
    mute: bool,
) -> Result<()> {
    tracing::debug!(sink_input_idx = idx, mute = mute, "setting sink-input mute via libpulse");

    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .set_sink_input_mute(
            idx,
            mute,
            Some(Box::new(move |success| {
                tracing::trace!(sink_input_idx = idx, success = success, "set_sink_input_mute callback fired");
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
pub fn move_sink_input_sync(
    conn: &mut PulseConnection,
    idx: u32,
    sink_name: &str,
) -> Result<()> {
    tracing::debug!(sink_input_idx = idx, sink_name = %sink_name, "moving sink-input via libpulse");

    let done: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    let _op = conn
        .context_mut()
        .introspect()
        .move_sink_input_by_name(
            idx,
            sink_name,
            Some(Box::new(move |success| {
                tracing::trace!(sink_input_idx = idx, success = success, "move_sink_input callback fired");
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

// ---------------------------------------------------------------------------
// Loopback sink-input discovery
// ---------------------------------------------------------------------------

/// Find the sink-input index belonging to a specific module, using the introspect API.
///
/// Returns `None` if no sink-input with the given `owner_module` is currently registered.
/// The caller is expected to retry with an inter-attempt delay when the module was just
/// loaded, since PipeWire may take a moment to register the sink-input.
#[instrument(skip(conn))]
pub fn find_sink_input_by_module_sync(
    conn: &mut PulseConnection,
    module_id: u32,
) -> Result<Option<u32>> {
    tracing::debug!(module_id = module_id, "finding sink-input by module via libpulse");

    // Collect (sink_input_index, owner_module) for every sink-input.
    let entries: Arc<Mutex<Vec<(u32, u32)>>> = Arc::new(Mutex::new(Vec::new()));
    let entries_clone = Arc::clone(&entries);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    conn.context_mut()
        .introspect()
        .get_sink_input_info_list(move |list_result| match list_result {
            ListResult::Item(info) => {
                if let Some(owner) = info.owner_module {
                    tracing::trace!(
                        sink_input_idx = info.index,
                        owner_module = owner,
                        "got sink-input owner_module"
                    );
                    entries_clone.lock().unwrap().push((info.index, owner));
                }
            }
            ListResult::End => {
                tracing::trace!("sink-input list complete (module search) — signalling mainloop");
                unsafe { (*ml_ptr).signal(false) };
            }
            ListResult::Error => {
                tracing::warn!("get_sink_input_info_list returned error during module search");
                unsafe { (*ml_ptr).signal(false) };
            }
        });

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let entries = Arc::try_unwrap(entries)
        .expect("no other Arc holders after wait")
        .into_inner()
        .unwrap();

    let found = entries
        .into_iter()
        .find(|(_, owner)| *owner == module_id)
        .map(|(idx, _)| idx);

    tracing::debug!(module_id = module_id, found = ?found, "sink-input module search complete");
    Ok(found)
}

// ---------------------------------------------------------------------------
// Sink-name filter (mirrors the pactl-based DeviceEnumerator logic)
// ---------------------------------------------------------------------------

const OSG_SINK_PREFIX: &str = "osg_";
const SINK_EXCLUDE_PATTERNS: &[&str] = &["_Apps", "_OBS"];

fn should_exclude_sink(name: &str) -> bool {
    if name.starts_with(OSG_SINK_PREFIX) {
        tracing::debug!(name = %name, reason = "osg prefix", "excluding virtual sink");
        return true;
    }
    let excluded = SINK_EXCLUDE_PATTERNS.iter().any(|pat| name.contains(pat));
    if excluded {
        tracing::trace!(name = %name, "filtering out virtual sink by pattern");
    }
    excluded
}
