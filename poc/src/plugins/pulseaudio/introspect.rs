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
use tracing::instrument;

use crate::error::Result;
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
                results_clone.lock().unwrap().push(RawSink {
                    index: info.index,
                    name,
                    description,
                });
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

    tracing::debug!(
        total_raw = raw.len(),
        "raw sources received from introspect"
    );

    raw.into_iter()
        .filter(|s| {
            if s.is_monitor {
                tracing::trace!(name = %s.name, "excluding monitor source");
                return false;
            }
            // Exclude OSG virtual sinks and EasyEffects/PipeWire virtual sources
            if s.name.starts_with("osg_") || s.name.contains("easyeffects") {
                tracing::trace!(name = %s.name, "excluding virtual source");
                return false;
            }
            true
        })
        .map(|s| HardwareInput {
            id: s.index,
            name: shorten_device_name(&s.description),
            description: s.description,
            device_id: s.name,
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

    conn.context_mut().introspect().get_sink_input_info_list(
        move |list_result| match list_result {
            ListResult::Item(info) => {
                let app_name = info.proplist.get_str(properties::APPLICATION_NAME);
                let app_binary = info
                    .proplist
                    .get_str(properties::APPLICATION_PROCESS_BINARY);
                let icon_name = info.proplist.get_str(properties::APPLICATION_ICON_NAME);
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
        },
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let raw = Arc::try_unwrap(results)
        .expect("no other Arc holders after wait")
        .into_inner()
        .unwrap();

    tracing::debug!(
        total_raw = raw.len(),
        "raw sink-inputs received from introspect"
    );

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
// Loopback sink-input discovery
// ---------------------------------------------------------------------------

/// Find the sink-input index belonging to a specific module, using the introspect API.
///
/// Primary lookup: match by `owner_module`. Fallback (PipeWire compat): if no
/// sink-input has a valid `owner_module` matching the module_id, match by the
/// sink index that the sink-input is connected to (`target_sink_idx`).
///
/// PipeWire's PA compatibility layer often sets `owner_module` to `None`,
/// which causes the primary lookup to fail silently. The sink-based fallback
/// ensures volume control works on PipeWire.
#[instrument(skip(conn))]
pub fn find_sink_input_by_module_sync(
    conn: &mut PulseConnection,
    module_id: u32,
) -> Result<Option<u32>> {
    find_sink_input_by_module_or_sink_sync(conn, module_id, None)
}

/// Extended sink-input finder: tries `owner_module` first, then falls back to
/// matching by `target_sink_idx` (the PA sink index the loopback connects to).
#[instrument(skip(conn))]
pub fn find_sink_input_by_module_or_sink_sync(
    conn: &mut PulseConnection,
    module_id: u32,
    target_sink_idx: Option<u32>,
) -> Result<Option<u32>> {
    tracing::debug!(
        module_id, target_sink_idx = ?target_sink_idx,
        "finding sink-input by module (primary) or sink (fallback)"
    );

    // Collect (sink_input_index, owner_module_opt, sink_idx) for every sink-input.
    #[derive(Debug)]
    struct Entry {
        index: u32,
        owner_module: Option<u32>,
        sink: u32,
    }
    let entries: Arc<Mutex<Vec<Entry>>> = Arc::new(Mutex::new(Vec::new()));
    let entries_clone = Arc::clone(&entries);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    conn.context_mut().introspect().get_sink_input_info_list(
        move |list_result| match list_result {
            ListResult::Item(info) => {
                tracing::trace!(
                    sink_input_idx = info.index,
                    owner_module = ?info.owner_module,
                    sink = info.sink,
                    name = ?info.name,
                    "sink-input enumerated"
                );
                entries_clone.lock().unwrap().push(Entry {
                    index: info.index,
                    owner_module: info.owner_module,
                    sink: info.sink,
                });
            }
            ListResult::End => {
                tracing::trace!("sink-input list complete — signalling mainloop");
                unsafe { (*ml_ptr).signal(false) };
            }
            ListResult::Error => {
                tracing::warn!("get_sink_input_info_list returned error during module search");
                unsafe { (*ml_ptr).signal(false) };
            }
        },
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let entries = Arc::try_unwrap(entries)
        .expect("no other Arc holders after wait")
        .into_inner()
        .unwrap();

    let total = entries.len();
    let with_owner = entries.iter().filter(|e| e.owner_module.is_some()).count();
    tracing::debug!(
        total_sink_inputs = total, with_owner_module = with_owner,
        "sink-input enumeration complete"
    );

    // Primary: match by owner_module
    if let Some(found) = entries
        .iter()
        .find(|e| e.owner_module == Some(module_id))
        .map(|e| e.index)
    {
        tracing::debug!(
            module_id, sink_input_idx = found,
            "found sink-input by owner_module (primary)"
        );
        return Ok(Some(found));
    }

    tracing::debug!(
        module_id, with_owner,
        "owner_module match failed — trying sink-based fallback"
    );

    // Fallback: match by target sink index (for PipeWire where owner_module is None)
    if let Some(target_sink) = target_sink_idx {
        if let Some(found) = entries
            .iter()
            .find(|e| e.sink == target_sink && e.owner_module.is_none())
            .map(|e| e.index)
        {
            tracing::info!(
                module_id, target_sink, sink_input_idx = found,
                "found sink-input by target sink (PipeWire fallback)"
            );
            return Ok(Some(found));
        }
    }

    tracing::warn!(
        module_id, target_sink_idx = ?target_sink_idx, total_sink_inputs = total,
        "sink-input not found by module or sink — volume control unavailable"
    );
    Ok(None)
}

/// Resolve a sink name to its PA sink index.
#[instrument(skip(conn))]
pub fn resolve_sink_index_by_name(
    conn: &mut PulseConnection,
    sink_name: &str,
) -> Result<Option<u32>> {
    tracing::debug!(sink_name = %sink_name, "resolving sink index by name");

    let result: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let result_clone = Arc::clone(&result);
    let ml_ptr: *mut Mainloop = conn.mainloop_mut();

    conn.mainloop_mut().lock();

    conn.context_mut().introspect().get_sink_info_by_name(
        sink_name,
        move |list_result| match list_result {
            ListResult::Item(info) => {
                tracing::trace!(sink_name = ?info.name, sink_idx = info.index, "resolved sink");
                *result_clone.lock().unwrap() = Some(info.index);
            }
            ListResult::End => {
                unsafe { (*ml_ptr).signal(false) };
            }
            ListResult::Error => {
                tracing::warn!("get_sink_info_by_name returned error");
                unsafe { (*ml_ptr).signal(false) };
            }
        },
    );

    conn.mainloop_mut().wait();
    conn.mainloop_mut().unlock();

    let idx = result.lock().unwrap().take();
    tracing::debug!(sink_name = %sink_name, resolved_idx = ?idx, "sink index resolution complete");
    Ok(idx)
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

/// Shorten a PA device description for sidebar display.
///
/// Removes common suffixes: "Analog Stereo", "Digital Stereo", "Pro Audio".
/// Truncates manufacturer prefixes if result is still long.
fn shorten_device_name(desc: &str) -> String {
    let mut name = desc.to_string();
    for suffix in &[
        " Analog Stereo",
        " Digital Stereo",
        " Pro Audio",
        " Multichannel",
    ] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped.to_string();
        }
    }
    // If still over 30 chars, truncate
    if name.len() > 30 {
        name.truncate(27);
        name.push_str("...");
    }
    name
}
