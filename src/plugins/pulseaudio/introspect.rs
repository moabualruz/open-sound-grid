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
